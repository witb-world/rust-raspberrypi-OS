// Note: use `emmc_transfer_blocks` for read/write.

// use alloc::vec::Vec;

use super::EMMCController;
use crate::{
    bsp::driver::get_emmc,
    driver,
    exception::asynchronous::IRQNumber,
    // memory::{Address, Physical},
    synchronization::{interface::Mutex, IRQSafeNullLock},
};

struct SDInner {
    emmc: &'static EMMCController,
    initialized: bool,
}

// const SECTOR_SIZE: u32 = 512;

//--------------------------------------------------------------------------------------------------
// Public Definitions
//--------------------------------------------------------------------------------------------------

/// Representation of SD interface to EMMC device
pub struct SD {
    // coming soon!
    inner: IRQSafeNullLock<SDInner>,
}

//--------------------------------------------------------------------------------------------------
// Private Code
//--------------------------------------------------------------------------------------------------
impl SDInner {
    /// Create an instance
    pub unsafe fn new() -> Self {
        // EMMC_CONTROLLER.
        // let emmc_start: Address<Virtual> = Address::new(0xFE34_0000);
        Self {
            // emmc: &EMMCController::new(emmc_start),
            // emmc: &EMMCController::new(EMMC_START),
            emmc: get_emmc(),
            initialized: false,
        }
    }

    /// initialize EMMC card reader
    fn emmc_init(&mut self) -> Result<(), &'static str> {
        // TODO: we must actually ~instantiate~ emmc before
        // trying to initialize it.
        // otherwise we'll get a kernel panic, trying to write to memory
        // that's never been allocated/instantiated
        self.emmc.emmc_init_card();
        self.initialized = true;
        Ok(())
    }

    fn emmc_read_sectors(&mut self, lba: u32, nsec: u32) -> Result<[u8; 512], &'static str> {
        // may just have to allocate then read in
        // this will require calculating size from nsec.

        // probhably an issue with using vec type here.

        let mut buffer: [u8; 512] = [0; 512];
        self.emmc
            .emmc_transfer_blocks(lba, nsec, &mut buffer, false);
        // println!("About to print end of buffer");
        // println!("{}", buffer[510]);
        Ok(buffer)
    }
}

//--------------------------------------------------------------------------------------------------
// Public Code
//--------------------------------------------------------------------------------------------------

impl SD {
    pub const COMPATIBLE: &'static str = "SD Driver";

    /// Create an instance
    pub unsafe fn new() -> Self {
        Self {
            inner: IRQSafeNullLock::new(SDInner::new()),
        }
    }

    /// initialize the SD driver
    pub fn pi_sd_init(&self) -> Result<(), &'static str> {
        self.inner.lock(|inner| inner.emmc_init())
    }

    // /// read in `nsec` of sectors starting at `lba` to buffer
    // pub fn pi_sd_read(buf: Vec<u8>, lba: u32, nsec: u32) -> Result<(), &'static str> {
    //     // coming soon!
    //     // see if we can assert that sd has been initialized
    //     Ok(())
    // }

    /// read `nsec` of sectors starting at `lba`, return buf
    pub fn pi_sec_read(&self, lba: u32, nsec: u32) -> Result<[u8; 512], &'static str> {
        let buffer = self.inner.lock(|inner| inner.emmc_read_sectors(lba, nsec));
        buffer
    }

    // /// write data to `nsec` sectors of SD card starting at `lba`
    // pub fn pi_sd_write(buf: Vec<u8>, lba: u32, nsec: u32) -> Result<(), &'static str> {
    //     // coming soon!
    //     Ok(())
    // }
}

//------------------------------------------------------------------------------
// OS Interface Code
//------------------------------------------------------------------------------
// use synchronization::interface::Mutex;

impl driver::interface::DeviceDriver for SD {
    type IRQNumberType = IRQNumber;

    fn compatible(&self) -> &'static str {
        Self::COMPATIBLE
    }
}
