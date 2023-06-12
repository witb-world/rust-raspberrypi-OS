// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Copyright (c) 2018-2022 Andre Richter <andre.o.richter@gmail.com>

// Rust embedded logo for `make doc`.
#![doc(
    html_logo_url = "https://raw.githubusercontent.com/rust-embedded/wg/master/assets/logo/ewg-logo-blue-white-on-transparent.png"
)]

//! The `kernel` binary.

#![feature(format_args_nl)]
#![no_main]
#![no_std]

extern crate alloc;

use libkernel::{bsp, cpu, driver, exception, info, memory, state, time};

/// Early init code.
///
/// When this code runs, virtual memory is already enabled.
///
/// # Safety
///
/// - Only a single core must be active and running this function.
/// - Printing will not work until the respective driver's MMIO is remapped.
#[no_mangle]
unsafe fn kernel_init() -> ! {
    exception::handling_init();
    memory::init();

    // Initialize the timer subsystem.
    if let Err(x) = time::init() {
        panic!("Error initializing timer subsystem: {}", x);
    }

    // Initialize the BSP driver subsystem.
    if let Err(x) = bsp::driver::init() {
        panic!("Error initializing BSP driver subsystem: {}", x);
    }

    // Initialize all device drivers.
    driver::driver_manager().init_drivers_and_irqs();

    bsp::memory::mmu::kernel_add_mapping_records_for_precomputed();

    // Unmask interrupts on the boot CPU core.
    exception::asynchronous::local_irq_unmask();

    // Announce conclusion of the kernel_init() phase.
    state::state_manager().transition_to_single_core_main();

    // Transition from unsafe to safe.
    kernel_main()
}

/// The main function running after the early init.
fn kernel_main() -> ! {
    use alloc::boxed::Box;
    use core::time::Duration;

    info!("{}", libkernel::version());
    info!("Booting on: {}", bsp::board_name());

    info!("MMU online:");
    memory::mmu::kernel_print_mappings();

    let (_, privilege_level) = exception::current_privilege_level();
    info!("Current privilege level: {}", privilege_level);

    info!("Exception handling state:");
    exception::asynchronous::print_state();

    info!(
        "Architectural timer resolution: {} ns",
        time::time_manager().resolution().as_nanos()
    );

    info!("Drivers loaded:");
    driver::driver_manager().enumerate();

    info!("Registered IRQ handlers:");
    exception::asynchronous::irq_manager().print_handler();

    info!("Kernel heap:");
    memory::heap_alloc::kernel_heap_allocator().print_usage();

    time::time_manager().set_timeout_once(Duration::from_secs(5), Box::new(|| info!("Once 5")));
    time::time_manager().set_timeout_once(Duration::from_secs(3), Box::new(|| info!("Once 2")));
    time::time_manager()
        .set_timeout_periodic(Duration::from_secs(1), Box::new(|| info!("Periodic 1 sec")));

    info!("Turning on GPIO 21");
    let gpio = bsp::driver::get_gpio();
    let mbr = bsp::driver::get_mbr();
    // let emmc = bsp::driver::get_emmc();

    let s = mbr.say_hello();
    info!("{}", s);

    info!("Attempting to INIT emmc");
    // emmc.emmc_init_card();
    // theoretically this will produce some debug output to the console...
    // emmc.emmc_debug_response(emmc.emmc_read_scr());

    gpio.set_output_pin(21);
    info!("Set output GPIO 21");

    // let sd = bsp::driver::get_sd();
    // let init_res = sd.pi_sd_init();
    // match init_res {
    //     Ok(()) => info!("Successfully initiated EMMC device"),
    //     _ => info!("Something went wrong during init"),
    // };

    // let part = mbr.mbr_get_partition(1);

    // info!("{}", part.mbr_partition_string());
    // info!(
    //     "More partition data: nsec: {}\tpart_type: {}",
    //     part.mbr_get_nsectors(),
    //     part.mbr_get_parttype()
    // );
    // info!("Full partition: {}", part);

    // info!("MBR sigval: {:x}", mbr.mbr_get_sigval());

    // info!("MBR check output: {}", mbr.mbr_check());

    let fat32 = bsp::driver::get_fat32();

    info!("Checking Fat32 vol");
    fat32.fat32_vol_id_check();
    info!("Check succeeded!");
    // let buf = sd.pi_sec_read(0, 1).unwrap();

    // info!("Got sd card MBR contents. End of block: ");
    // info!("[{}, {}]", buf[510], buf[511]);
    // match sd_read {
    //     Ok(buf) => info!("[{}, {}]", buf[510], buf[511]),
    //     _ => info!("An error may have occured while trying to read from SD card."),
    // }

    info!("Spinning for 3 second");
    gpio.set_pin_on(21);
    time::time_manager().spin_for(Duration::from_secs(3));
    info!("Spinning again for 1 second");
    gpio.set_pin_off(21);
    time::time_manager().spin_for(Duration::from_secs(1));

    info!("Echoing input now");
    cpu::wait_forever();
}
