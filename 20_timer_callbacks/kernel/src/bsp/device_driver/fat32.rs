// use super::libkernel::info;
use crate::{
    bsp::{
        device_driver::PartitionEntry,
        driver::{get_mbr, get_sd},
    },
    // debug,
    driver,
    exception::asynchronous::IRQNumber,
    println,
    synchronization,
    synchronization::IRQSafeNullLock,
};
use alloc::{
    string::{String, ToString},
    vec::Vec,
};
use core::str;
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
#[derive(Debug)]
pub struct Dirent {
    // may want to enforce that these two fields max out at 16 bytes...
    name: String,
    raw_name: String,

    cluster_id: u32,
    nbytes: u32,
    is_dir_p: bool,
}

#[allow(dead_code)]
#[derive(Deserialize, Debug)]
struct Fat32Dirent {
    filename: [u8; 11], // 8 byte filename, 3 byte ext
    attr: u8,           // bitvector for attributes
    reserved: u8,
    create_time_tenths: u8,
    create_time: [u8; 2],
    create_date: [u8; 2],
    access_date: [u8; 2],
    hi_start: [u8; 2],
    mod_time: [u8; 2],
    mod_date: [u8; 2],
    lo_start: [u8; 2],
    file_nbytes: [u8; 4],
}
#[derive(PartialEq)]
#[allow(dead_code)]

enum Fat32DirentAttrs {
    Fat32ReadOnly = 0x01,
    Fat32Hidden = 0x02,
    Fat32SystemFile = 0x04,
    Fat32VolumeLabel = 0x08,
    Fat32LongFileName = 0x0f,
    Fat32Dir = 0x10,
    Fat32Archive = 0x20,
}

impl Fat32Dirent {
    pub fn get_dirent_attr(&self, attr: Fat32DirentAttrs) -> bool {
        match attr {
            Fat32DirentAttrs::Fat32ReadOnly => self.attr & 0x01 != 0,
            Fat32DirentAttrs::Fat32Hidden => self.attr & 0x02 != 0,
            Fat32DirentAttrs::Fat32SystemFile => self.attr & 0x04 != 0,
            Fat32DirentAttrs::Fat32VolumeLabel => self.attr & 0x08 != 0,
            Fat32DirentAttrs::Fat32LongFileName => self.attr & 0x0f != 0,
            Fat32DirentAttrs::Fat32Dir => self.attr & 0x10 != 0,
            Fat32DirentAttrs::Fat32Archive => self.attr & 0x20 != 0,
        }
    }

    pub fn dirent_is_lfn(&self) -> bool {
        self.get_dirent_attr(Fat32DirentAttrs::Fat32LongFileName)
    }

    pub fn dirent_is_vol_label(&self) -> bool {
        self.attr & 0x08 != 0
    }

    pub fn dirent_is_free(&self) -> bool {
        let x: u8 = self.filename[0];
        x == 0 || x == 0xe5
    }

    pub fn dirent_cluster_id(&self) -> u32 {
        let hi_start_16: u16 = arr_to_u16(self.hi_start);
        let lo_start_16: u16 = arr_to_u16(self.lo_start);
        let hi_start: u32 = u32::try_from(hi_start_16).unwrap();
        let lo_start: u32 = u32::try_from(lo_start_16).unwrap();

        hi_start << 16 | lo_start
    }

    pub fn dirent_convert(&self) -> Dirent {
        // let mut filename: [u8; 11] = [0; 11];
        // filename.copy_from_slice(self.filename.as_ref());
        // let filename = self.filename;
        let name = core::str::from_utf8(&self.filename).unwrap().to_string();
        let raw_name = core::str::from_utf8(&self.filename).unwrap().to_string();

        // let raw_name = name;
        Dirent {
            name: name, // should invoke a helper to convert from raw to presentable
            raw_name: raw_name,
            cluster_id: self.dirent_cluster_id(),
            is_dir_p: self.get_dirent_attr(Fat32DirentAttrs::Fat32Dir),
            nbytes: arr_to_u32(self.file_nbytes),
        }
    }
}

#[allow(dead_code)]
const DIRENT_CHECKER: [u8; 32] = [0; core::mem::size_of::<Fat32Dirent>()];
#[derive(Debug)]
#[allow(dead_code)]
pub struct Directory {
    dirents: Vec<Dirent>,
    n_dirents: u32,
}

#[derive(PartialEq)]
enum Fat32ClusterType {
    FreeCluster = 0,
    ReservedCluster = 1,
    BadCluster = 0xFFF_FFF7,
    LastCluster,
    UsedCluster,
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

pub fn arr_to_u32_le(arr: [u8; 4]) -> u32 {
    let mut res = 0;
    for i in 0..4 {
        res += u32::try_from(arr[i]).unwrap() << (32 - i * 8);
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
        let mut info_sector_vec: Vec<u8> = Vec::new();
        info_sector_vec.resize(512, 0);

        info_sector_vec = sd.pi_sec_read(lba_start + if_sec_num, 1).unwrap();
        // println!("Boot_sec: bytes_per_sec: {}", boot_sec.bytes_per_sec);
        println!(
            "FSInfo sig3: [{},{},{},{}]",
            info_sector_vec[508], info_sector_vec[509], info_sector_vec[510], info_sector_vec[511]
        );
        let info = FSInfo::new(info_sector_vec);
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

    pub fn cluster_to_lba(&self, cluster_num: u32) -> u32 {
        assert!(cluster_num >= 2);
        self.clusters_begin_lba + (cluster_num - 2) * self.sectors_per_cluster
    }

    pub fn get_fat_entry_type(&self, x: u32) -> Fat32ClusterType {
        let mut cls = x;
        cls = (cls << 4) >> 4; // clear upper bits
        println!(
            "Attempting to match cluster type: {:x}, derived from {:x}",
            cls, x
        );
        match cls {
            0x0 => Fat32ClusterType::FreeCluster,
            0x1 => Fat32ClusterType::ReservedCluster,
            0xFFF_FFF7 => Fat32ClusterType::BadCluster,
            0xFFF_FFF8..=0xFFF_FFFF => Fat32ClusterType::LastCluster,
            // 0xFFF_FFF9 => Fat32ClusterType::UsedCluster,
            0x2..=0xFFF_FFEF => Fat32ClusterType::UsedCluster,
            _ => panic!("Reserved value matched in cluster"),
        }
    }

    fn get_next_cluster_val(&self, last_cluster_idx: u32) -> u32 {
        let idx: usize = usize::try_from(last_cluster_idx).unwrap();
        // uh oh, this index is oob!
        println!(
            "Getting next value, first index is {}. size of fat is {}",
            idx,
            self.fat.len()
        );
        println!(
            "Indices are [{}, {}, {}, {}]",
            self.fat[idx],
            self.fat[idx + 1],
            self.fat[idx + 2],
            self.fat[idx + 3],
        );
        let val_arr = [
            self.fat[idx],
            self.fat[idx + 1],
            self.fat[idx + 2],
            self.fat[idx + 3],
        ];
        // (val_arr[0] << 24) | (val_arr[1] << 16) | (val_arr[2] << 8) | (val_arr[3])

        arr_to_u32_le(val_arr)
    }

    pub fn get_cluster_chain_length(&self, start_cluster: u32) -> u32 {
        let mut chain_length: u32 = 0;
        let mut cluster: u32 = start_cluster;
        // loop, checking FAT entry type.
        while self.get_fat_entry_type(cluster) != Fat32ClusterType::LastCluster
            && self.get_fat_entry_type(cluster) != Fat32ClusterType::FreeCluster
            && self.get_fat_entry_type(cluster) != Fat32ClusterType::BadCluster
            && self.get_fat_entry_type(cluster) != Fat32ClusterType::ReservedCluster
        {
            // let mut next_cluster_idx = self.fat[usize::try_from(cluster).unwrap()];
            println!("Getting next cluster value from current value {}", cluster);
            cluster = self.get_next_cluster_val(cluster);
            chain_length += 1;
        }
        chain_length
    }

    pub fn get_cluster_chain_data(&self, start_cluster: u32) -> Vec<u8> {
        let mut data: Vec<u8> = Vec::new();
        let mut cluster: u32 = start_cluster;
        let read_size = self.sectors_per_cluster;

        while self.get_fat_entry_type(cluster) != Fat32ClusterType::LastCluster
            && self.get_fat_entry_type(cluster) != Fat32ClusterType::FreeCluster
            && self.get_fat_entry_type(cluster) != Fat32ClusterType::BadCluster
            && self.get_fat_entry_type(cluster) != Fat32ClusterType::ReservedCluster
        {
            // chain_length += 1;
            let mut new_data: Vec<u8> = Vec::new();
            new_data.resize(512, 0);
            new_data = self
                .sd
                .pi_sec_read(self.cluster_to_lba(cluster), read_size)
                .unwrap();
            let new_data_slice = new_data.as_slice();

            data = [data.as_slice(), new_data_slice].concat();
            cluster = self.get_next_cluster_val(cluster);
        }
        data
    }

    fn get_dirents(&self, start_cluster: u32) -> Vec<Fat32Dirent> {
        let _chain_len = self.get_cluster_chain_length(start_cluster);
        let mut dirent_vec: Vec<Fat32Dirent> = Vec::new();
        let mut cluster_chain_data: Vec<u8> = Vec::new();
        cluster_chain_data.resize(usize::try_from(_chain_len).unwrap() * 512, 0);
        cluster_chain_data = self.get_cluster_chain_data(start_cluster);
        // how can we effectively "memcpy" this data?
        let dirent_size = 32; // 32 bytes per dirent

        // iterate through chain data, creating new dirent and appending to vec at each step.
        for i in (0..cluster_chain_data.len() - dirent_size).step_by(dirent_size) {
            // println!("Adding Fat32Dirent...");
            let dir_ent: Fat32Dirent = from_bytes(&cluster_chain_data[i..i + dirent_size]).unwrap();
            dirent_vec.push(dir_ent);
        }
        dirent_vec
    }

    fn fat32_get_root(&self) -> Dirent {
        let first_cluster = self.root_dir_first_cluster;

        Dirent {
            name: "root".to_string(),
            raw_name: "root".to_string(),
            cluster_id: first_cluster,
            is_dir_p: true,
            nbytes: 0,
        }
    }

    fn read_dir(&self, dir_ent: Dirent) -> Directory {
        let fat32_dirent_vec = self.get_dirents(dir_ent.cluster_id);
        let mut dirents: Vec<Dirent> = Vec::new();

        let mut num_valid_dirents: u32 = 0;
        // let mut j: usize = 0;
        println!("Number of dirents: {}", fat32_dirent_vec.len());
        for i in 0..fat32_dirent_vec.len() {
            // add to dirents
            let this_entry = &fat32_dirent_vec[i];
            if this_entry.dirent_is_lfn()
                || this_entry.dirent_is_free()
                || this_entry.dirent_is_vol_label()
            {
                continue;
            };

            num_valid_dirents += 1;
            dirents.push(this_entry.dirent_convert());
        }

        Directory {
            dirents: (dirents),
            n_dirents: (num_valid_dirents),
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

    pub fn fat32_get_root(&self) -> Dirent {
        self.inner.lock(|inner| inner.fat32_get_root())
    }

    pub fn fat32_read_dir(&self, dirent: Dirent) -> Directory {
        self.inner.lock(|inner| inner.read_dir(dirent))
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
