// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Copyright (c) 2018-2022 Andre Richter <andre.o.richter@gmail.com>

//! GPIO Driver.

use crate::{
    bsp::device_driver::common::MMIODerefWrapper,
    driver,
    exception::asynchronous::IRQNumber,
    memory::{Address, Virtual},
    synchronization,
    synchronization::IRQSafeNullLock,
};
use tock_registers::{
    interfaces::{ReadWriteable, Writeable},
    register_bitfields, register_structs,
    registers::ReadWrite,
};

//--------------------------------------------------------------------------------------------------
// Private Definitions
//--------------------------------------------------------------------------------------------------

// GPIO registers.
//
// Descriptions taken from
// - https://github.com/raspberrypi/documentation/files/1888662/BCM2837-ARM-Peripherals.-.Revised.-.V2-1.pdf
// - https://datasheets.raspberrypi.org/bcm2711/bcm2711-peripherals.pdf
// register_gpio!(21);

register_bitfields! {
    u32,
    /// GPIO Function Select 1
    GPFSEL1 [
        /// Pin 15
        FSEL15 OFFSET(15) NUMBITS(3) [
            Input = 0b000,
            Output = 0b001,
            AltFunc0 = 0b100  // PL011 UART RX

        ],

        /// Pin 14
        FSEL14 OFFSET(12) NUMBITS(3) [
            Input = 0b000,
            Output = 0b001,
            AltFunc0 = 0b100  // PL011 UART TX
        ]
    ],

    /// GPIO Function Select 2
    GPFSEL2 [
        /// Pin 20
        FSEL20 OFFSET(0) NUMBITS(3) [
            Input = 0b000,
            Output = 0b001,
            AltFunc0 = 0b100
        ],
        /// Pin 21
        FSEL21 OFFSET(3) NUMBITS(3) [
            Input = 0b000,
            Output = 0b001,
            AltFunc0 = 0b100
        ]
    ],

    /// GPIO Set 0
    GPSET0 [
        /// Pin 20
        SET OFFSET(0) NUMBITS(32) [
            Set0 = 1,
            Set1 = 1 << 1,
            Set2 = 1 << 2,
            Set3 = 1 << 3,
            Set4 = 1 << 4,
            Set5 = 1 << 5,
            Set6 = 1 << 6,
            Set7 = 1 << 7,
            Set8 = 1 << 8,
            Set9 = 1 << 9,
            Set10 = 1 << 10,
            Set11 = 1 << 11,
            Set12 = 1 << 12,
            Set13 = 1 << 13,
            Set14 = 1 << 14,
            Set15 = 1 << 15,
            Set16 = 1 << 16,
            Set17 = 1 << 17,
            Set18 = 1 << 18,
            Set19 = 1 << 19,
            Set20 = 1 << 20,
            Set21 = 1 << 21,
            Set22 = 1 << 22,
            Set23 = 1 << 23,
            Set24 = 1 << 24,
            Set25 = 1 << 25,
            Set26 = 1 << 26,
            Set27 = 1 << 27,
            Set28 = 1 << 28,
            Set29 = 1 << 29,
            Set30 = 1 << 30,
            Set31 = 1 << 31
            // Set = 1,       // see BCM2711 pg. 70
            // NotSet = 0 // note that we don't actually clear with this register.
        ]
    ],

    /// GPIO Clear 0
    GPCLR0 [
        /// Pin 20
        CLR OFFSET(0) NUMBITS(32) [
            Clr0 = 1,
            Clr1 = 1 << 1,
            Clr2 = 1 << 2,
            Clr3 = 1 << 3,
            Clr4 = 1 << 4,
            Clr5 = 1 << 5,
            Clr6 = 1 << 6,
            Clr7 = 1 << 7,
            Clr8 = 1 << 8,
            Clr9 = 1 << 9,
            Clr10 = 1 << 10,
            Clr11 = 1 << 11,
            Clr12 = 1 << 12,
            Clr13 = 1 << 13,
            Clr14 = 1 << 14,
            Clr15 = 1 << 15,
            Clr16 = 1 << 16,
            Clr17 = 1 << 17,
            Clr18 = 1 << 18,
            Clr19 = 1 << 19,
            Clr20 = 1 << 20,
            Clr21 = 1 << 21,
            Clr22 = 1 << 22,
            Clr23 = 1 << 23,
            Clr24 = 1 << 24,
            Clr25 = 1 << 25,
            Clr26 = 1 << 26,
            Clr27 = 1 << 27,
            Clr28 = 1 << 28,
            Clr29 = 1 << 29,
            Clr30 = 1 << 30,
            Clr31 = 1 << 31,
        ]
    ],
    /// GPIO Pull-up/down Register
    ///
    /// BCM2837 only.
    GPPUD [
        /// Controls the actuation of the internal pull-up/down control line to ALL the GPIO pins.
        PUD OFFSET(0) NUMBITS(2) [
            Off = 0b00,
            PullDown = 0b01,
            PullUp = 0b10
        ]
    ],

    /// GPIO Pull-up/down Clock Register 0
    ///
    /// BCM2837 only.
    GPPUDCLK0 [
        /// Pin 15
        PUDCLK15 OFFSET(15) NUMBITS(1) [
            NoEffect = 0,
            AssertClock = 1
        ],

        /// Pin 14
        PUDCLK14 OFFSET(14) NUMBITS(1) [
            NoEffect = 0,
            AssertClock = 1
        ]
    ],

    /// GPIO Pull-up / Pull-down Register 0
    ///
    /// BCM2711 only.
    GPIO_PUP_PDN_CNTRL_REG0 [
        /// Pin 15
        GPIO_PUP_PDN_CNTRL15 OFFSET(30) NUMBITS(2) [
            NoResistor = 0b00,
            PullUp = 0b01
        ],

        /// Pin 14
        GPIO_PUP_PDN_CNTRL14 OFFSET(28) NUMBITS(2) [
            NoResistor = 0b00,
            PullUp = 0b01
        ]
    ]
}

register_structs! {
    #[allow(non_snake_case)]
    RegisterBlock {
        (0x00 => _reserved1),
        (0x04 => GPFSEL1: ReadWrite<u32, GPFSEL1::Register>),
        (0x08 => GPFSEL2: ReadWrite<u32, GPFSEL2::Register>),
        (0x0C => _reserved2),
        (0x1C => GPSET0: ReadWrite<u32, GPSET0::Register>),
        (0x20 => _reserved3),
        (0x28 => GPCLR0: ReadWrite<u32, GPCLR0::Register>),
        (0x2C => _reserved4), // this would be occupied by GPCLR1
        (0x94 => GPPUD: ReadWrite<u32, GPPUD::Register>),
        (0x98 => GPPUDCLK0: ReadWrite<u32, GPPUDCLK0::Register>),
        (0x9C => _reserved5),
        (0xE4 => GPIO_PUP_PDN_CNTRL_REG0: ReadWrite<u32, GPIO_PUP_PDN_CNTRL_REG0::Register>),
        (0xE8 => @END),
    }
}

/// Abstraction for the associated MMIO registers.
type Registers = MMIODerefWrapper<RegisterBlock>;

struct GPIOInner {
    registers: Registers,
}

//--------------------------------------------------------------------------------------------------
// Public Definitions
//--------------------------------------------------------------------------------------------------

/// Representation of the GPIO HW.
pub struct GPIO {
    inner: IRQSafeNullLock<GPIOInner>,
}

//--------------------------------------------------------------------------------------------------
// Private Code
//--------------------------------------------------------------------------------------------------

impl GPIOInner {
    /// Create an instance.
    ///
    /// # Safety
    ///
    /// - The user must ensure to provide a correct MMIO start address.
    pub const unsafe fn new(mmio_start_addr: Address<Virtual>) -> Self {
        Self {
            registers: Registers::new(mmio_start_addr),
        }
    }

    /// Disable pull-up/down on pins 14 and 15.
    #[cfg(feature = "bsp_rpi3")]
    fn disable_pud_14_15_bcm2837(&mut self) {
        use crate::time;
        use core::time::Duration;

        // The Linux 2837 GPIO driver waits 1 µs between the steps.
        const DELAY: Duration = Duration::from_micros(1);

        self.registers.GPPUD.write(GPPUD::PUD::Off);
        time::time_manager().spin_for(DELAY);

        self.registers
            .GPPUDCLK0
            .write(GPPUDCLK0::PUDCLK15::AssertClock + GPPUDCLK0::PUDCLK14::AssertClock);
        time::time_manager().spin_for(DELAY);

        self.registers.GPPUD.write(GPPUD::PUD::Off);
        self.registers.GPPUDCLK0.set(0);
    }

    /// Disable pull-up/down on pins 14 and 15.
    #[cfg(feature = "bsp_rpi4")]
    fn disable_pud_14_15_bcm2711(&mut self) {
        self.registers.GPIO_PUP_PDN_CNTRL_REG0.write(
            GPIO_PUP_PDN_CNTRL_REG0::GPIO_PUP_PDN_CNTRL15::PullUp
                + GPIO_PUP_PDN_CNTRL_REG0::GPIO_PUP_PDN_CNTRL14::PullUp,
        );
    }

    /// Map PL011 UART as standard output.
    ///
    /// TX to pin 14
    /// RX to pin 15
    pub fn map_pl011_uart(&mut self) {
        // Select the UART on pins 14 and 15.
        self.registers
            .GPFSEL1
            .modify(GPFSEL1::FSEL15::AltFunc0 + GPFSEL1::FSEL14::AltFunc0);

        // Disable pull-up/down on pins 14 and 15.
        #[cfg(feature = "bsp_rpi3")]
        self.disable_pud_14_15_bcm2837();

        #[cfg(feature = "bsp_rpi4")]
        self.disable_pud_14_15_bcm2711();
    }

    pub fn map_pin_output(&mut self, pin: u32) {
        // remove constraint after adding more GPIO registers
        assert!(pin == 20 || pin == 21);
        match pin {
            20 => self.registers.GPFSEL2.modify(GPFSEL2::FSEL20::Output),
            21 => self.registers.GPFSEL2.modify(GPFSEL2::FSEL21::Output),
            _ => panic!("invalid register"),
        };
    }

    pub fn turn_pin_on(&mut self, pin: u32) {
        assert!(pin < 32);
        match pin {
            0 => self.registers.GPSET0.modify(GPSET0::SET::Set0),
            1 => self.registers.GPSET0.modify(GPSET0::SET::Set1),
            2 => self.registers.GPSET0.modify(GPSET0::SET::Set2),
            3 => self.registers.GPSET0.modify(GPSET0::SET::Set3),
            4 => self.registers.GPSET0.modify(GPSET0::SET::Set4),
            5 => self.registers.GPSET0.modify(GPSET0::SET::Set5),
            6 => self.registers.GPSET0.modify(GPSET0::SET::Set6),
            7 => self.registers.GPSET0.modify(GPSET0::SET::Set7),
            8 => self.registers.GPSET0.modify(GPSET0::SET::Set8),
            9 => self.registers.GPSET0.modify(GPSET0::SET::Set9),
            10 => self.registers.GPSET0.modify(GPSET0::SET::Set10),
            11 => self.registers.GPSET0.modify(GPSET0::SET::Set11),
            12 => self.registers.GPSET0.modify(GPSET0::SET::Set12),
            13 => self.registers.GPSET0.modify(GPSET0::SET::Set13),
            14 => self.registers.GPSET0.modify(GPSET0::SET::Set14),
            15 => self.registers.GPSET0.modify(GPSET0::SET::Set15),
            16 => self.registers.GPSET0.modify(GPSET0::SET::Set16),
            17 => self.registers.GPSET0.modify(GPSET0::SET::Set17),
            18 => self.registers.GPSET0.modify(GPSET0::SET::Set18),
            19 => self.registers.GPSET0.modify(GPSET0::SET::Set19),
            20 => self.registers.GPSET0.modify(GPSET0::SET::Set20),
            21 => self.registers.GPSET0.modify(GPSET0::SET::Set21),
            22 => self.registers.GPSET0.modify(GPSET0::SET::Set22),
            23 => self.registers.GPSET0.modify(GPSET0::SET::Set23),
            24 => self.registers.GPSET0.modify(GPSET0::SET::Set24),
            25 => self.registers.GPSET0.modify(GPSET0::SET::Set25),
            26 => self.registers.GPSET0.modify(GPSET0::SET::Set26),
            27 => self.registers.GPSET0.modify(GPSET0::SET::Set27),
            28 => self.registers.GPSET0.modify(GPSET0::SET::Set28),
            29 => self.registers.GPSET0.modify(GPSET0::SET::Set29),
            30 => self.registers.GPSET0.modify(GPSET0::SET::Set30),
            31 => self.registers.GPSET0.modify(GPSET0::SET::Set31),
            _ => panic!("invalid register"),
        };
    }

    pub fn turn_pin_off(&mut self, pin: u32) {
        assert!(pin < 32);
        match pin {
            0 => self.registers.GPCLR0.modify(GPCLR0::CLR::Clr0),
            1 => self.registers.GPCLR0.modify(GPCLR0::CLR::Clr1),
            2 => self.registers.GPCLR0.modify(GPCLR0::CLR::Clr2),
            3 => self.registers.GPCLR0.modify(GPCLR0::CLR::Clr3),
            4 => self.registers.GPCLR0.modify(GPCLR0::CLR::Clr4),
            5 => self.registers.GPCLR0.modify(GPCLR0::CLR::Clr5),
            6 => self.registers.GPCLR0.modify(GPCLR0::CLR::Clr6),
            7 => self.registers.GPCLR0.modify(GPCLR0::CLR::Clr7),
            8 => self.registers.GPCLR0.modify(GPCLR0::CLR::Clr8),
            9 => self.registers.GPCLR0.modify(GPCLR0::CLR::Clr9),
            10 => self.registers.GPCLR0.modify(GPCLR0::CLR::Clr10),
            11 => self.registers.GPCLR0.modify(GPCLR0::CLR::Clr11),
            12 => self.registers.GPCLR0.modify(GPCLR0::CLR::Clr12),
            13 => self.registers.GPCLR0.modify(GPCLR0::CLR::Clr13),
            14 => self.registers.GPCLR0.modify(GPCLR0::CLR::Clr14),
            15 => self.registers.GPCLR0.modify(GPCLR0::CLR::Clr15),
            16 => self.registers.GPCLR0.modify(GPCLR0::CLR::Clr16),
            17 => self.registers.GPCLR0.modify(GPCLR0::CLR::Clr17),
            18 => self.registers.GPCLR0.modify(GPCLR0::CLR::Clr18),
            19 => self.registers.GPCLR0.modify(GPCLR0::CLR::Clr19),
            20 => self.registers.GPCLR0.modify(GPCLR0::CLR::Clr20),
            21 => self.registers.GPCLR0.modify(GPCLR0::CLR::Clr21),
            22 => self.registers.GPCLR0.modify(GPCLR0::CLR::Clr22),
            23 => self.registers.GPCLR0.modify(GPCLR0::CLR::Clr23),
            24 => self.registers.GPCLR0.modify(GPCLR0::CLR::Clr24),
            25 => self.registers.GPCLR0.modify(GPCLR0::CLR::Clr25),
            26 => self.registers.GPCLR0.modify(GPCLR0::CLR::Clr26),
            27 => self.registers.GPCLR0.modify(GPCLR0::CLR::Clr27),
            28 => self.registers.GPCLR0.modify(GPCLR0::CLR::Clr28),
            29 => self.registers.GPCLR0.modify(GPCLR0::CLR::Clr29),
            30 => self.registers.GPCLR0.modify(GPCLR0::CLR::Clr30),
            31 => self.registers.GPCLR0.modify(GPCLR0::CLR::Clr31),
            _ => panic!("invalid register"),
        };
    }
}

//--------------------------------------------------------------------------------------------------
// Public Code
//--------------------------------------------------------------------------------------------------

impl GPIO {
    pub const COMPATIBLE: &'static str = "BCM GPIO";

    /// Create an instance.
    ///
    /// # Safety
    ///
    /// - The user must ensure to provide a correct MMIO start address.
    pub const unsafe fn new(mmio_start_addr: Address<Virtual>) -> Self {
        Self {
            inner: IRQSafeNullLock::new(GPIOInner::new(mmio_start_addr)),
        }
    }

    /// Concurrency safe version of `GPIOInner.map_pl011_uart()`
    pub fn map_pl011_uart(&self) {
        self.inner.lock(|inner| inner.map_pl011_uart())
    }

    // early stage PoC: call some new code of my own from GPIO driver
    pub fn say_hello(&self) -> &'static str {
        "Hello, World!"
    }

    /// set provided pin to GPIO output
    pub fn set_output_pin(&self, pin: u32) {
        self.inner.lock(|inner| inner.map_pin_output(pin))
    }

    /// turn GPIO pin on
    pub fn set_pin_on(&self, pin: u32) {
        self.inner.lock(|inner| inner.turn_pin_on(pin))
    }

    /// turn GPIO pin off
    pub fn set_pin_off(&self, pin: u32) {
        self.inner.lock(|inner| inner.turn_pin_off(pin))
    }
}

//------------------------------------------------------------------------------
// OS Interface Code
//------------------------------------------------------------------------------
use synchronization::interface::Mutex;

impl driver::interface::DeviceDriver for GPIO {
    type IRQNumberType = IRQNumber;

    fn compatible(&self) -> &'static str {
        Self::COMPATIBLE
    }
}
