// use super::libkernel::info;
use crate::{
    bsp::{
        device_driver::PartitionEntry,
        driver::{get_mbr, get_sd},
    },
    driver,
    exception::asynchronous::IRQNumber,
    println, synchronization,
    synchronization::IRQSafeNullLock,
};
use alloc::{string::String, vec::Vec};
use postcard::from_bytes;
use serde::Deserialize;

#[allow(dead_code)]

struct File {
    data: Vec<u8>,
}

#[derive(Deserialize, Debug)]
#[allow(dead_code)]

pub struct BootSector {
    asm_code: [u8; 3],
    oem: [u8; 8],
    bytes_per_sec: [u8; 2],
    sec_per_cluster: u8,
    reserved_area_nsec: [u8; 2],
    nfats: u8,
    max_files: [u8; 2],
    fs_nsec: [u8; 2],
    media_type: u8,
    zero: [u8; 2],
    sec_per_track: [u8; 2],
    n_heads: [u8; 2],
    hidden_secs: [u8; 4],
    nsec_in_fs: [u8; 4],
    nsec_per_fat: [u8; 4],
    mirror_flags: [u8; 2],
    version: [u8; 2],
    first_cluster: [u8; 4],
    info_sec_num: [u8; 2],
    backup_boot_loc: [u8; 2],
    _reserved: [u8; 12],
    logical_drive_num: u8,
    _reserved1: u8,
    extended_sig: u8,
    serial_num: [u8; 4],
    volume_label: [u8; 11],
    fs_type: [u8; 8],
    // Unfortunately serde::deserialize doesn't allow for arrays longer than this...
    _ignore: [u8; 32],
    _ignore1: [u8; 32],
    _ignore2: [u8; 32],
    _ignore3: [u8; 32],
    _ignore4: [u8; 32],
    _ignore5: [u8; 32],
    _ignore6: [u8; 32],
    _ignore7: [u8; 32],
    _ignore8: [u8; 32],
    _ignore9: [u8; 32],
    _ignore_a: [u8; 32],
    _ignore_b: [u8; 32],
    _ignore_c: [u8; 32],
    _ignore_d: [u8; 4],
    sig: [u8; 2],
}

// a hacky static-assert for size of BootSector struct
#[allow(dead_code)]
const TEST_CHECKER: [u8; 512] = [0; core::mem::size_of::<BootSector>()];
#[allow(dead_code)]

struct Dirent {
    // may want to enforce that these two fields max out at 16 bytes...
    name: String,
    raw_name: String,

    cluster_id: u32,
    nbytes: u32,
    is_dir_p: bool,
}
#[allow(dead_code)]

struct Directory {
    dirents: Vec<Dirent>,
    n_dirents: u32,
}
#[allow(dead_code)]

struct Fat32Inner {
    lba_start: u32,
    fat_begin_lba: u32,
    clusters_begin_lba: u32,
    sectors_per_cluster: u32,
    root_dir_first_cluster: u32,
    // pointer to in-memory copy of FAT: use a vector of bytes?
    fat: Vec<u8>,
    n_entries: u32,

    info: FSInfo,
    boot_sec: BootSector,

    sd: &'static SD,
}

//--------------------------------------------------------------------------------------------------
// Private Code
//--------------------------------------------------------------------------------------------------
// use bincode::deserialize;

// unline the struct above, we won't fill in `ignore` fields.
#[derive(Debug)]
#[allow(dead_code)]
pub struct FSInfo {
    sig1: u32,
    sig2: u32,
    free_cluster_count: u32,
    next_free_cluster: u32,
    sig3: u32,
}

impl FSInfo {
    pub fn new(info_sector: Vec<u8>) -> Self {
        println!("Info sector [488]: {}", info_sector[488]);
        Self {
            sig1: sl_to_u32(&info_sector[0..4]),
            sig2: sl_to_u32(&info_sector[484..488]),
            free_cluster_count: sl_to_u32(&info_sector[488..492]),
            next_free_cluster: sl_to_u32(&info_sector[492..496]),
            sig3: sl_to_u32(&info_sector[508..512]),
        }
    }
}

pub fn arr_to_u32(arr: [u8; 4]) -> u32 {
    let mut res = 0;
    for i in 0..4 {
        res += u32::try_from(arr[i]).unwrap() << (i * 8);
    }
    res
}

pub fn sl_to_u32(sl: &[u8]) -> u32 {
    let mut res = 0;
    for i in 0..4 {
        res += u32::try_from(sl[i]).unwrap() << (i * 8);
    }
    res
}

pub fn arr_to_u16(arr: [u8; 2]) -> u16 {
    let mut res = 0;
    for i in 0..2 {
        res += u16::try_from(arr[i]).unwrap() << (i * 8);
    }
    res
}

impl Fat32Inner {
    pub fn new(partition: PartitionEntry, sd: &'static SD) -> Self {
        // need to use lba_start of partition to read in boot_sector.
        let lba_start = partition.mbr_get_lba_start();
        let mut boot_sector_vec: Vec<u8> = Vec::new();
        boot_sector_vec.resize(512, 0);
        boot_sector_vec = sd.pi_sec_read(lba_start, 1).unwrap();
        // then need to "memcpy" this vec into BootSector type. Use `bincode` crate.
        println!(
            "BootSector[10..13]:\t{}, {}, {}, {}",
            boot_sector_vec[10], boot_sector_vec[11], boot_sector_vec[12], boot_sector_vec[13]
        );
        let boot_sector_arr: [u8; 512] = boot_sector_vec.try_into().unwrap_or_else(|v: Vec<u8>| {
            panic!("Expected a Vec of length {} but it was {}", 512, v.len());
        });
        let boot_sec: BootSector = from_bytes(&boot_sector_arr).unwrap();
        println!("BootSector: {:#?}", boot_sec);
        let if_sec_num = u32::try_from(arr_to_u16(boot_sec.info_sec_num)).unwrap();
        println!("Got Info sector number: {}", if_sec_num);
        let info = FSInfo::new(sd.pi_sec_read(if_sec_num, 1).unwrap());
        // println!("Boot_sec: bytes_per_sec: {}", boot_sec.bytes_per_sec);

        println!("FSInfo: {:#?}", info);
        let fat_begin_lba =
            lba_start + u32::try_from(arr_to_u16(boot_sec.reserved_area_nsec)).unwrap();
        let cluster_begin_lba = fat_begin_lba
            + u32::try_from(boot_sec.nfats).unwrap() * arr_to_u32(boot_sec.nsec_per_fat);
        let n_entries = arr_to_u32(boot_sec.nsec_per_fat) * 512 / 4;

        let fat: Vec<u8> = Vec::new();
        // fat.resize(usize::try_from(n_entries).unwrap() * 4, 0);
        // fat = sd
        //     .pi_sec_read(fat_begin_lba, arr_to_u32(boot_sec.nsec_per_fat))
        //     .unwrap();

        Self {
            lba_start: lba_start,
            fat_begin_lba: fat_begin_lba,
            clusters_begin_lba: cluster_begin_lba,
            sectors_per_cluster: u32::try_from(boot_sec.sec_per_cluster).unwrap(),
            root_dir_first_cluster: arr_to_u32(boot_sec.first_cluster),
            n_entries: n_entries,
            sd: &sd,
            fat: fat,
            boot_sec: boot_sec,
            info: info,
        }
    }

    /// Read in FAT table
    #[allow(dead_code)]
    pub fn fat32_read_fat(&mut self) {
        println!(
            "Reading FAT, from {} with {} sectors.",
            self.fat_begin_lba,
            arr_to_u32(self.boot_sec.nsec_per_fat)
        );
        let fat = self
            .sd
            .pi_sec_read(self.fat_begin_lba, arr_to_u32(self.boot_sec.nsec_per_fat));

        match fat {
            Ok(fat) => self.fat = fat,
            _ => panic!("Something went wrong while reading FAT"),
        };
    }

    pub fn fat32_volume_id_check(&self) {
        println!("Bytes per sec: {}", arr_to_u16(self.boot_sec.bytes_per_sec));
        assert!(arr_to_u16(self.boot_sec.bytes_per_sec) == 512);
        assert!(self.boot_sec.nfats == 2);
        assert!(arr_to_u16(self.boot_sec.sig) == 0xAA55);

        // TODO: replace check below with power-of-two check
        assert!(self.boot_sec.sec_per_cluster % 2 == 0);

        assert!(arr_to_u16(self.boot_sec.max_files) == 0);
        assert!(arr_to_u16(self.boot_sec.fs_nsec) == 0);
        assert!(arr_to_u16(self.boot_sec.zero) == 0);
        assert!(arr_to_u32(self.boot_sec.nsec_in_fs) != 0);

        assert!(arr_to_u16(self.boot_sec.info_sec_num) == 1);
        assert!(arr_to_u16(self.boot_sec.backup_boot_loc) == 6);
        assert!(self.boot_sec.extended_sig == 0x29);
    }
}
//--------------------------------------------------------------------------------------------------
// Public Definitions
//--------------------------------------------------------------------------------------------------
#[allow(dead_code)]
pub struct Fat32 {
    inner: IRQSafeNullLock<Fat32Inner>,
}

//--------------------------------------------------------------------------------------------------
// Public Code
//--------------------------------------------------------------------------------------------------

#[allow(dead_code)]
impl Fat32 {
    pub unsafe fn new() -> Result<Self, &'static str> {
        println!("~~~~~~~~~~~~~~~~~~~~~~~~IN FAT32 CONSTRUCTOR~~~~~~~~~~~~~~~~~~~~~~~~~");
        let sd_driver = get_sd();
        // have to call MBR constructor, presumably?
        let mbr = get_mbr();
        match sd_driver.pi_sd_init() {
            Ok(()) => println!("SD Init successful"),
            _ => panic!("Couldn't init SD"),
        }
        let first_partition = mbr.mbr_get_partition(1);
        println!(
            "~~~~~~~PARTITION INFO~~~~~~~~~{}~~~~~~PARTITION INFO~~~~~~~~",
            first_partition
        );
        Ok(Self {
            inner: IRQSafeNullLock::new(Fat32Inner::new(first_partition, sd_driver)),
        })
    }

    pub const COMPATIBLE: &'static str = "FAT32";

    pub fn fat32_vol_id_check(&self) {
        self.inner.lock(|inner| inner.fat32_volume_id_check())
    }

    pub fn fat32_read_fat(&self) {
        self.inner.lock(|inner| inner.fat32_read_fat())
    }
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
