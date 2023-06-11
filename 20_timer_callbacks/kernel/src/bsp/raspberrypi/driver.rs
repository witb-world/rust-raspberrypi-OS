// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Copyright (c) 2018-2022 Andre Richter <andre.o.richter@gmail.com>

//! BSP driver support.

use super::{exception, memory::map::mmio};
use crate::{
    bsp::device_driver,
    console, driver as generic_driver,
    exception::{self as generic_exception},
    memory,
    memory::mmu::MMIODescriptor,
};
use core::{
    mem::MaybeUninit,
    sync::atomic::{AtomicBool, Ordering},
};

//--------------------------------------------------------------------------------------------------
// Global instances
//--------------------------------------------------------------------------------------------------

static mut PL011_UART: MaybeUninit<device_driver::PL011Uart> = MaybeUninit::uninit();
static mut GPIO: MaybeUninit<device_driver::GPIO> = MaybeUninit::uninit();
static mut MBR: MaybeUninit<device_driver::MBR> = MaybeUninit::uninit();
static mut EMMC_CONTROLLER: MaybeUninit<device_driver::EMMCController> = MaybeUninit::uninit();
static mut SD: MaybeUninit<device_driver::SD> = MaybeUninit::uninit();

#[cfg(feature = "bsp_rpi3")]
static mut INTERRUPT_CONTROLLER: MaybeUninit<device_driver::InterruptController> =
    MaybeUninit::uninit();

#[cfg(feature = "bsp_rpi4")]
static mut INTERRUPT_CONTROLLER: MaybeUninit<device_driver::GICv2> = MaybeUninit::uninit();

//--------------------------------------------------------------------------------------------------
// Private Code
//--------------------------------------------------------------------------------------------------

/// This must be called only after successful init of the memory subsystem.
unsafe fn instantiate_uart() -> Result<(), &'static str> {
    let mmio_descriptor = MMIODescriptor::new(mmio::PL011_UART_START, mmio::PL011_UART_SIZE);
    let virt_addr =
        memory::mmu::kernel_map_mmio(device_driver::PL011Uart::COMPATIBLE, &mmio_descriptor)?;

    PL011_UART.write(device_driver::PL011Uart::new(virt_addr));

    Ok(())
}

/// This must be called only after successful init of the UART driver.
unsafe fn post_init_uart() -> Result<(), &'static str> {
    console::register_console(PL011_UART.assume_init_ref());

    Ok(())
}

/// This must be called only after successful init of the memory subsystem.
unsafe fn instantiate_gpio() -> Result<(), &'static str> {
    let mmio_descriptor = MMIODescriptor::new(mmio::GPIO_START, mmio::GPIO_SIZE);
    let virt_addr =
        memory::mmu::kernel_map_mmio(device_driver::GPIO::COMPATIBLE, &mmio_descriptor)?;

    GPIO.write(device_driver::GPIO::new(virt_addr));

    Ok(())
}

/// This must be called only after successful init of the GPIO driver.
unsafe fn post_init_gpio() -> Result<(), &'static str> {
    GPIO.assume_init_ref().map_pl011_uart();
    Ok(())
}

/// This must be called only after successful init of the memory subsystem.
unsafe fn instantiate_emmc() -> Result<(), &'static str> {
    let mmio_descriptor = MMIODescriptor::new(mmio::EMMC_START, mmio::EMMC_SIZE);
    let virt_addr =
        memory::mmu::kernel_map_mmio(device_driver::EMMCController::COMPATIBLE, &mmio_descriptor)?;
    EMMC_CONTROLLER.write(device_driver::EMMCController::new(virt_addr));
    // EMMC_CONTROLLER
    Ok(())
}

/// This must be called only after successful init of EMMC driver
unsafe fn post_init_emmc() -> Result<(), &'static str> {
    EMMC_CONTROLLER.assume_init_ref();
    Ok(())
}

unsafe fn instantiate_sd() -> Result<(), &'static str> {
    // let mmio_descriptor
    SD.write(device_driver::SD::new());
    Ok(())
}

unsafe fn post_init_sd() -> Result<(), &'static str> {
    SD.assume_init_ref();
    Ok(())
}

unsafe fn instantiate_mbr() -> Result<(), &'static str> {
    MBR.write(device_driver::MBR::new()?);
    Ok(())
}

unsafe fn post_init_mbr() -> Result<(), &'static str> {
    MBR.assume_init_ref();
    Ok(())
}

/// This must be called only after successful init of the memory subsystem.
#[cfg(feature = "bsp_rpi3")]
unsafe fn instantiate_interrupt_controller() -> Result<(), &'static str> {
    let local_mmio_descriptor = MMIODescriptor::new(mmio::LOCAL_IC_START, mmio::LOCAL_IC_SIZE);
    let local_virt_addr = memory::mmu::kernel_map_mmio(
        device_driver::InterruptController::COMPATIBLE,
        &local_mmio_descriptor,
    )?;

    let periph_mmio_descriptor =
        MMIODescriptor::new(mmio::PERIPHERAL_IC_START, mmio::PERIPHERAL_IC_SIZE);
    let periph_virt_addr = memory::mmu::kernel_map_mmio(
        device_driver::InterruptController::COMPATIBLE,
        &periph_mmio_descriptor,
    )?;

    INTERRUPT_CONTROLLER.write(device_driver::InterruptController::new(
        local_virt_addr,
        periph_virt_addr,
    ));

    Ok(())
}

/// This must be called only after successful init of the memory subsystem.
#[cfg(feature = "bsp_rpi4")]
unsafe fn instantiate_interrupt_controller() -> Result<(), &'static str> {
    let gicd_mmio_descriptor = MMIODescriptor::new(mmio::GICD_START, mmio::GICD_SIZE);
    let gicd_virt_addr = memory::mmu::kernel_map_mmio("GICv2 GICD", &gicd_mmio_descriptor)?;

    let gicc_mmio_descriptor = MMIODescriptor::new(mmio::GICC_START, mmio::GICC_SIZE);
    let gicc_virt_addr = memory::mmu::kernel_map_mmio("GICV2 GICC", &gicc_mmio_descriptor)?;

    INTERRUPT_CONTROLLER.write(device_driver::GICv2::new(gicd_virt_addr, gicc_virt_addr));

    Ok(())
}

/// This must be called only after successful init of the interrupt controller driver.
unsafe fn post_init_interrupt_controller() -> Result<(), &'static str> {
    generic_exception::asynchronous::register_irq_manager(INTERRUPT_CONTROLLER.assume_init_ref());

    Ok(())
}

/// Function needs to ensure that driver registration happens only after correct instantiation.
unsafe fn driver_uart() -> Result<(), &'static str> {
    instantiate_uart()?;

    let uart_descriptor = generic_driver::DeviceDriverDescriptor::new(
        PL011_UART.assume_init_ref(),
        Some(post_init_uart),
        Some(exception::asynchronous::irq_map::PL011_UART),
    );
    generic_driver::driver_manager().register_driver(uart_descriptor);

    Ok(())
}

/// Function needs to ensure that driver registration happens only after correct instantiation.
unsafe fn driver_gpio() -> Result<(), &'static str> {
    instantiate_gpio()?;

    let gpio_descriptor = generic_driver::DeviceDriverDescriptor::new(
        GPIO.assume_init_ref(),
        Some(post_init_gpio),
        None,
    );
    generic_driver::driver_manager().register_driver(gpio_descriptor);

    Ok(())
}

/// Function needs to ensure that driver registration happens only after correct instantiation.
unsafe fn driver_emmc_controller() -> Result<(), &'static str> {
    instantiate_emmc()?;

    let emmc_descriptor = generic_driver::DeviceDriverDescriptor::new(
        EMMC_CONTROLLER.assume_init_ref(),
        Some(post_init_emmc),
        None,
    );
    generic_driver::driver_manager().register_driver(emmc_descriptor);
    Ok(())
}

unsafe fn driver_sd() -> Result<(), &'static str> {
    instantiate_sd()?;
    let sd_descriptor =
        generic_driver::DeviceDriverDescriptor::new(SD.assume_init_ref(), Some(post_init_sd), None);
    generic_driver::driver_manager().register_driver(sd_descriptor);
    Ok(())
}

unsafe fn driver_mbr() -> Result<(), &'static str> {
    instantiate_mbr()?;
    let mbr_descriptor = generic_driver::DeviceDriverDescriptor::new(
        MBR.assume_init_ref(),
        Some(post_init_mbr),
        None,
    );
    generic_driver::driver_manager().register_driver(mbr_descriptor);
    Ok(())
}

/// Function needs to ensure that driver registration happens only after correct instantiation.
unsafe fn driver_interrupt_controller() -> Result<(), &'static str> {
    instantiate_interrupt_controller()?;

    let interrupt_controller_descriptor = generic_driver::DeviceDriverDescriptor::new(
        INTERRUPT_CONTROLLER.assume_init_ref(),
        Some(post_init_interrupt_controller),
        None,
    );
    generic_driver::driver_manager().register_driver(interrupt_controller_descriptor);

    Ok(())
}

//--------------------------------------------------------------------------------------------------
// Public Code
//--------------------------------------------------------------------------------------------------

/// Initialize the driver subsystem.
///
/// # Safety
///
/// See child function calls.
pub unsafe fn init() -> Result<(), &'static str> {
    static INIT_DONE: AtomicBool = AtomicBool::new(false);
    if INIT_DONE.load(Ordering::Relaxed) {
        return Err("Init already done");
    }

    driver_uart()?;
    driver_gpio()?;
    driver_interrupt_controller()?;
    driver_emmc_controller()?;
    driver_sd()?;
    driver_mbr()?;

    INIT_DONE.store(true, Ordering::Relaxed);
    Ok(())
}

/// Return a reference to GPIO driver
pub fn get_gpio() -> &'static device_driver::GPIO {
    // static INIT_DONE: AtomicBool = AtomicBool::new(false);
    // if !INIT_DONE.load(Ordering::Relaxed) {
    //     return Err("Not initialized yet.");
    // )
    unsafe { GPIO.assume_init_ref() }
}

/// Return a reference to EMMC driver
pub fn get_emmc() -> &'static device_driver::EMMCController {
    unsafe { EMMC_CONTROLLER.assume_init_ref() }
}

/// Return a reference to SD driver
pub fn get_sd() -> &'static device_driver::SD {
    unsafe { SD.assume_init_ref() }
}

/// Return a reference to MBR driver
pub fn get_mbr() -> &'static device_driver::MBR {
    unsafe { MBR.assume_init_ref() }
}

/// Minimal code needed to bring up the console in QEMU (for testing only). This is often less steps
/// than on real hardware due to QEMU's abstractions.
#[cfg(feature = "test_build")]
pub fn qemu_bring_up_console() {
    use crate::cpu;

    unsafe {
        instantiate_uart().unwrap_or_else(|_| cpu::qemu_exit_failure());
        console::register_console(PL011_UART.assume_init_ref());
    };
}
