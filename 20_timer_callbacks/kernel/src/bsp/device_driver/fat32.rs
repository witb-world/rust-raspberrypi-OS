use crate::{
    bsp::{
        device_driver::PartitionEntry,
        driver::{get_mbr, get_sd},
    },
    driver,
    exception::asynchronous::IRQNumber,
    synchronization,
    synchronization::IRQSafeNullLock,
};
use alloc::{string::String, vec::Vec};

struct File {
    data: Vec<u8>,
}

struct BootSector {
    asm_code: [u8; 3],
    oem: [u8; 8],
    bytes_per_sec: u16,
    sec_per_cluster: u8,
    reserved_area_nsec: u16,
    nfats: u8,
    max_files: u16,
    fs_nsec: u16,
    media_type: u8,
    zero: u16,
    sec_per_track: u16,
    n_heads: u16,
    hidden_secs: u32,
    nsec_in_fs: u32,
    nsec_per_fat: u32,
    mirror_flags: u16,
    version: u16,
    first_cluster: u32,
    info_sec_num: u16,
    backup_boot_loc: u16,
    _reserved: [u8; 12],
    logical_drive_num: u8,
    _reserved1: u8,
    extended_sig: u8,
    serial_num: u32,
    volume_label: [u8; 11],
    fs_type: [u8; 8],
    _ignore: [u8; 420],
    sig: u16,
}
const TestChecker: [u8; 512] = [0; core::mem::size_of::<BootSector>()];

struct Dirent {
    // may want to enforce that these two fields max out at 16 bytes...
    name: String,
    raw_name: String,

    cluster_id: u32,
    nbytes: u32,
    is_dir_p: bool,
}

struct Directory {
    dirents: Vec<Dirent>,
    n_dirents: u32,
}
struct Fat32Inner {
    lba_start: u32,
    fat_begin_lba: u32,
    clusters_begin_lba: u32,
    sectors_per_cluster: u32,
    root_dir_first_cluster: u32,
    // pointer to in-memory copy of FAT: use a vector of bytes?
    fat: Vec<u8>,
    n_entries: u32,

    sd: SD,
}

//--------------------------------------------------------------------------------------------------
// Private Code
//--------------------------------------------------------------------------------------------------
use bincode::deserialize;

impl Fat32Inner {
    pub fn new(partition: PartitionEntry, sd: SD) -> Self {
        // need to use lba_start of partition to read in boot_sector.
        let boot_sector_vec = sd.pi_sec_read(partition.mbr_get_lba_start(), 1)?;
        // then need to "memcpy" this vec into BootSector type. Use `bincode` crate.

        let boot_sec: BootSector = deserialize(&boot_sector_vec).unwrap();
        Self {}
    }
}
//--------------------------------------------------------------------------------------------------
// Public Definitions
//--------------------------------------------------------------------------------------------------
pub struct Fat32 {
    inner: IRQSafeNullLock<Fat32Inner>,
}

//--------------------------------------------------------------------------------------------------
// Public Code
//--------------------------------------------------------------------------------------------------
impl Fat32 {
    pub unsafe fn new() -> Result<Self, &'static str> {
        let sd_driver = get_sd();
        let mbr = get_mbr();
        sd_driver.pi_sd_init();
        let first_partition = mbr.mbr_get_partition(1);
        Ok(Self {
            inner: IRQSafeNullLock::new(Fat32Inner::new(first_partition)),
        })
    }

    pub const COMPATIBLE: &'static str = "Fat32";
}

//------------------------------------------------------------------------------
// OS Interface Code
//------------------------------------------------------------------------------
use synchronization::interface::Mutex;

use super::SD;

impl driver::interface::DeviceDriver for Fat32 {
    type IRQNumberType = IRQNumber;

    fn compatible(&self) -> &'static str {
        Self::COMPATIBLE
    }
}
