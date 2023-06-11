//! MB driver top level.
use crate::{
    bsp::driver::get_sd, driver, exception::asynchronous::IRQNumber, synchronization,
    synchronization::IRQSafeNullLock,
};
use alloc::vec::Vec;

/// Abstraction for MBR:
/// Boot code
/// Partition table entry 1
/// Partition table entry 2
/// Partition table entry 3
/// Partition table entry 4
/// Signature Value
// struct MBRInner {
//     arb: u32,
// }

#[allow(dead_code)]
pub struct PartitionEntry {
    bootable_p: u8,
    chs_start: [u8; 3],
    part_type: u8,
    chs_end: [u8; 3],
    lba_start: u32,
    nsec: u32,
}
#[allow(dead_code)]
struct MBRInner {
    code: [u8; 446],
    part_tab1: [u8; 16],
    part_tab2: [u8; 16],
    part_tab3: [u8; 16],
    part_tab4: [u8; 16],
    sigval: u16,
}

//--------------------------------------------------------------------------------------------------
// Private Code
//--------------------------------------------------------------------------------------------------
impl PartitionEntry {
    pub fn new(part: [u8; 16]) -> Self {
        // let mut p_entry: PartitionEntry;
        let this_bootable_p: u8 = part[0];
        let mut this_chs_start: [u8; 3] = [0; 3];
        let this_part_type = part[4];
        let mut this_chs_end: [u8; 3] = [0; 3];
        // let this

        this_chs_start.copy_from_slice(&part[1..4]);
        this_chs_end.copy_from_slice(&part[5..8]);
        let mut lba_arr: [u8; 4] = [0; 4];
        let mut nsec_arr: [u8; 4] = [0; 4];
        lba_arr.copy_from_slice(&part[8..12]);
        nsec_arr.copy_from_slice(&part[12..16]);
        Self {
            bootable_p: this_bootable_p,
            chs_start: this_chs_start,
            part_type: this_part_type,
            chs_end: this_chs_end,
            // What endianness is the lba data? Assumming littleendian
            lba_start: u32::from_le_bytes(lba_arr),
            nsec: u32::from_le_bytes(nsec_arr),
        }
    }

    pub fn mbr_partition_string(&self) -> &str {
        "hello from the MBR partition!"
    }

    pub fn mbr_get_nsectors(&self) -> u32 {
        self.nsec
    }

    pub fn mbr_get_parttype(&self) -> u8 {
        self.part_type
    }
}

impl MBRInner {
    #[allow(dead_code)]
    pub unsafe fn new(boot_sector: Vec<u8>) -> Self {
        // may want to assert that length of boot_sector is in fact 512
        let mut this_code: [u8; 446] = [0; 446];
        let mut this_part_tab1: [u8; 16] = [0; 16];
        let mut this_part_tab2: [u8; 16] = [0; 16];
        let mut this_part_tab3: [u8; 16] = [0; 16];
        let mut this_part_tab4: [u8; 16] = [0; 16];
        // let mut this_sigval: u16 = 0;

        this_code.copy_from_slice(&boot_sector[0..446]);
        this_part_tab1.copy_from_slice(&boot_sector[446..446 + 16]);
        this_part_tab2.copy_from_slice(&boot_sector[446 + 16..446 + 32]);
        this_part_tab3.copy_from_slice(&boot_sector[446 + 32..446 + 48]);
        this_part_tab4.copy_from_slice(&boot_sector[446 + 48..446 + 64]);

        let sigval_high = u16::try_from(boot_sector[510]).unwrap();
        let this_sigval = sigval_high << 8 + boot_sector[511];

        Self {
            code: this_code,
            part_tab1: this_part_tab1,
            part_tab2: this_part_tab2,
            part_tab3: this_part_tab3,
            part_tab4: this_part_tab4,
            sigval: this_sigval,
        }
    }

    pub fn get_partition(&self, partno: u32) -> PartitionEntry {
        match partno {
            1 => PartitionEntry::new(self.part_tab1),
            2 => PartitionEntry::new(self.part_tab2),
            3 => PartitionEntry::new(self.part_tab3),
            4 => PartitionEntry::new(self.part_tab4),
            _ => panic!("Invalid partition number!"),
        }
    }

    pub fn get_sigval(&self) -> u16 {
        self.sigval
    }
}
//--------------------------------------------------------------------------------------------------
// Public Definitions
//--------------------------------------------------------------------------------------------------

pub struct MBR {
    inner: IRQSafeNullLock<MBRInner>,
}

//---
// Public code
//----
impl MBR {
    pub unsafe fn new() -> Result<Self, &'static str> {
        let sd_driver = get_sd();
        sd_driver.pi_sd_init()?;
        let boot_sector = sd_driver.pi_sec_read(0, 1).unwrap();
        assert!(boot_sector.len() == 512);
        Ok(Self {
            inner: IRQSafeNullLock::new(MBRInner::new(boot_sector)),
        })
    }
    pub const COMPATIBLE: &'static str = "MBR";
    /// Placeholder public code
    pub fn say_hello(&self) -> &'static str {
        // self.inner.arb = 0xdeadbeef;
        "Hello from the MBR reader!"
    }

    pub fn mbr_part_is_fat32(t: i32) -> bool {
        t == 0xBi32 || t == 0xCi32
    }

    pub fn mbr_get_partition(&self, partno: u32) -> PartitionEntry {
        self.inner.lock(|inner| inner.get_partition(partno))
    }

    pub fn mbr_get_sigval(&self) -> u16 {
        self.inner.lock(|inner| inner.get_sigval())
    }
}

//------------------------------------------------------------------------------
// OS Interface Code
//------------------------------------------------------------------------------
use synchronization::interface::Mutex;

impl driver::interface::DeviceDriver for MBR {
    type IRQNumberType = IRQNumber;

    fn compatible(&self) -> &'static str {
        Self::COMPATIBLE
    }
}
