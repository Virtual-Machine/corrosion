use crate::block;
use crate::buffer::Buffer;
use crate::memory::memcpy;
use crate::uart::serial_debug;
use crate::{print, println};
use core::mem::size_of;
use rust_alloc::{collections::BTreeMap, string::String};

const MAGIC: u16 = 0x4d5a;
const ROOT_NODE: u32 = 1;
const DIR_ENTRY_START: usize = 2;
const FILE_NAME_SIZE: usize = 60;
const SECTOR_SIZE: usize = 512;
pub const BLOCK_SIZE: u32 = 1024;
const PTR_INDEX_MAX: usize = BLOCK_SIZE as usize / 4;
const S_IFDIR: u16 = 0o040_000;
const DIRECT_ZONES: usize = 7;
const INDIRECT_ZONE: usize = 7;
const DOUBLE_INDIRECT_ZONE: usize = 8;
const TRIPLE_INDIRECT_ZONE: usize = 9;

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct SuperBlock {
    pub ninodes: u32,
    pub pad0: u16,
    pub imap_blocks: u16,
    pub zmap_blocks: u16,
    pub first_data_zone: u16,
    pub log_zone_size: u16,
    pub pad1: u16,
    pub max_size: u32,
    pub zones: u32,
    pub magic: u16,
    pub pad2: u16,
    pub block_size: u16,
    pub disk_version: u8,
}

impl SuperBlock {
    fn is_minixfs(&self) -> bool {
        self.magic == MAGIC
    }

    fn blocks_first_four_areas(&self) -> usize {
        (2 + self.imap_blocks + self.zmap_blocks) as usize
    }

    fn inode_offset(&self, inode_num: u32) -> usize {
        (inode_num as usize - 1) / (BLOCK_SIZE as usize / size_of::<Inode>())
    }

    fn inode_index(&self, inode_num: u32) -> usize {
        (inode_num as usize - 1) % (BLOCK_SIZE as usize / size_of::<Inode>())
    }

    fn inode_offset_and_index(&self, inode_num: u32) -> (usize, usize) {
        let offset = self.blocks_first_four_areas() * BLOCK_SIZE as usize
            + self.inode_offset(inode_num) * BLOCK_SIZE as usize;
        let index = self.inode_index(inode_num);
        (offset, index)
    }

    fn get_inode(&self, inode_num: u32) -> Option<Inode> {
        if self.is_minixfs() {
            let (inode_offset, inode_index) = self.inode_offset_and_index(inode_num);
            let mut inode_buffer = Buffer::default();
            let inode_ptr = inode_buffer.get_mut() as *mut Inode;
            block::read(inode_buffer.get_mut(), BLOCK_SIZE, inode_offset as u64);
            unsafe { Some(*(inode_ptr.add(inode_index))) }
        } else {
            println!("WARNING: Couldn't read superblock as expected");
            None
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct Inode {
    pub mode: u16,
    pub nlinks: u16,
    pub uid: u16,
    pub gid: u16,
    pub size: u32,
    pub atime: u32,
    pub mtime: u32,
    pub ctime: u32,
    pub zones: [u32; 10],
}

impl Inode {
    fn get_dirents(&self) -> (*const DirEntry, usize) {
        let mut buf = Buffer::new(((self.size + BLOCK_SIZE - 1) & !BLOCK_SIZE) as usize);
        let dirents = buf.get() as *const DirEntry;
        let sz = MinixFileSystem::read(self, buf.get_mut(), BLOCK_SIZE, 0);
        let num_dirents = sz as usize / size_of::<DirEntry>();
        (dirents, num_dirents)
    }

    fn is_directory(&self) -> bool {
        self.mode & S_IFDIR != 0
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct DirEntry {
    pub inode: u32,
    pub name: [u8; 60],
}

impl DirEntry {
    fn abs_name(&self, cwd: &str, inode_num: u32) -> String {
        let mut new_cwd = String::with_capacity(120);
        for i in cwd.bytes() {
            new_cwd.push(i as char);
        }
        if inode_num != 1 {
            new_cwd.push('/');
        }
        for i in 0..FILE_NAME_SIZE {
            if self.name[i] == 0 {
                break;
            }
            new_cwd.push(self.name[i] as char);
        }
        new_cwd.shrink_to_fit();
        new_cwd
    }
}

static mut MFS_INODE_CACHE: BTreeMap<String, Inode> = BTreeMap::new();
static mut SUPERBLOCK: SuperBlock = SuperBlock {
    ninodes: 0,
    pad0: 0,
    imap_blocks: 0,
    zmap_blocks: 0,
    first_data_zone: 0,
    log_zone_size: 0,
    pad1: 0,
    max_size: 0,
    zones: 0,
    magic: 0,
    pad2: 0,
    block_size: 0,
    disk_version: 0,
};

struct ReadState {
    offset_byte: u32,
    bytes_read: u32,
    bytes_left: u32,
    blocks_seen: u32,
    offset_block: u32,
    direct_buffer: Buffer,
    indirect_buffer: Buffer,
    double_indirect_buffer: Buffer,
    triple_indirect_buffer: Buffer,
    izones: *const u32,
    iizones: *const u32,
    iiizones: *const u32,
}

impl ReadState {
    fn new(inode_size: u32, size: u32, offset: u32) -> Self {
        let mut rs = Self {
            offset_byte: offset % BLOCK_SIZE,
            bytes_read: 0,
            bytes_left: if size > inode_size { inode_size } else { size },
            blocks_seen: 0,
            offset_block: offset / BLOCK_SIZE,
            direct_buffer: Buffer::default(),
            indirect_buffer: Buffer::default(),
            double_indirect_buffer: Buffer::default(),
            triple_indirect_buffer: Buffer::default(),
            izones: core::ptr::null(),
            iizones: core::ptr::null(),
            iiizones: core::ptr::null(),
        };
        rs.izones = rs.indirect_buffer.get() as *const u32;
        rs.iizones = rs.double_indirect_buffer.get() as *const u32;
        rs.iiizones = rs.triple_indirect_buffer.get() as *const u32;
        rs
    }
    fn next(&mut self, bytes_to_read: u32) {
        self.offset_byte = 0;
        self.bytes_read += bytes_to_read;
        self.bytes_left -= bytes_to_read;
    }

    fn seen_block(&mut self) {
        self.blocks_seen += 1;
    }

    fn in_window(&self) -> bool {
        self.offset_block <= self.blocks_seen
    }

    fn izone_present(&self, index: usize) -> bool {
        unsafe { self.izones.add(index).read() != 0 }
    }

    fn iizone_present(&self, index: usize) -> bool {
        unsafe { self.iizones.add(index).read() != 0 }
    }

    fn iiizone_present(&self, index: usize) -> bool {
        unsafe { self.iiizones.add(index).read() != 0 }
    }
}

pub struct MinixFileSystem;
impl MinixFileSystem {
    pub fn get_inode(inode_num: u32) -> Option<Inode> {
        unsafe { SUPERBLOCK.get_inode(inode_num) }
    }

    fn cache_tree(btm: &mut BTreeMap<String, Inode>, cwd: &str, inode_num: u32) {
        let inode = Self::get_inode(inode_num).expect("To be passed a valid inode_num");
        let (dirents, num_dirents) = inode.get_dirents();
        for i in DIR_ENTRY_START..num_dirents {
            let directory_entry = &(unsafe { *dirents.add(i) });
            let directory_entry_inode = Self::get_inode(directory_entry.inode).unwrap();
            let new_cwd = directory_entry.abs_name(cwd, inode_num);
            if directory_entry_inode.is_directory() {
                Self::cache_tree(btm, &new_cwd, directory_entry.inode);
            } else {
                btm.insert(new_cwd, directory_entry_inode);
            }
        }
    }

    fn init_superblock_cache() {
        let mut buffer = Buffer::new(SECTOR_SIZE);
        let super_block = unsafe { &*(buffer.get_mut() as *mut SuperBlock) };
        block::read(buffer.get_mut(), SECTOR_SIZE as u32, BLOCK_SIZE as u64);
        unsafe { SUPERBLOCK = *super_block };
    }

    fn init_inode_cache() {
        let mut btm = BTreeMap::new();
        let cwd = String::from("/");

        Self::cache_tree(&mut btm, &cwd, ROOT_NODE);
        unsafe { MFS_INODE_CACHE = btm };
    }

    pub fn init() {
        Self::init_superblock_cache();
        Self::init_inode_cache();
    }

    fn read_data(buffer: *mut u8, rs: &mut ReadState) {
        let bytes_to_read = if BLOCK_SIZE - rs.offset_byte > rs.bytes_left {
            rs.bytes_left
        } else {
            BLOCK_SIZE - rs.offset_byte
        };
        unsafe {
            memcpy(
                buffer.add(rs.bytes_read as usize),
                rs.direct_buffer.get().add(rs.offset_byte as usize),
                bytes_to_read as usize,
            );
        }
        rs.next(bytes_to_read);
    }

    fn read_direct_data(inode: &Inode, i: usize, buffer: *mut u8, rs: &mut ReadState) {
        let zone_offset = inode.zones[i] * BLOCK_SIZE;
        block::read(rs.direct_buffer.get_mut(), BLOCK_SIZE, zone_offset as u64);
        Self::read_data(buffer, rs);
    }

    fn read_indirect_data(izones: *const u32, i: usize, buffer: *mut u8, rs: &mut ReadState) {
        block::read(
            rs.direct_buffer.get_mut(),
            BLOCK_SIZE,
            (BLOCK_SIZE * unsafe { izones.add(i).read() }) as u64,
        );
        Self::read_data(buffer, rs);
    }

    fn read_zone(inode: &Inode, buffer: &mut Buffer, number: usize) {
        block::read(
            buffer.get_mut(),
            BLOCK_SIZE,
            (BLOCK_SIZE * inode.zones[number]) as u64,
        );
    }

    fn read_izone(izones: *const u32, buffer: &mut Buffer, i: usize) {
        block::read(
            buffer.get_mut(),
            BLOCK_SIZE,
            (BLOCK_SIZE * unsafe { izones.add(i).read() }) as u64,
        );
    }

    fn direct_zones(inode: &Inode, buffer: *mut u8, rs: &mut ReadState) -> u32 {
        for i in 0..DIRECT_ZONES {
            if inode.zones[i] == 0 {
                continue;
            }
            if rs.in_window() {
                Self::read_direct_data(inode, i, buffer, rs);
                if rs.bytes_left == 0 {
                    return rs.bytes_read;
                }
            }
            rs.seen_block()
        }
        0
    }

    fn indirect_zones(inode: &Inode, buffer: *mut u8, rs: &mut ReadState) -> u32 {
        if inode.zones[INDIRECT_ZONE] != 0 {
            Self::read_zone(inode, &mut rs.indirect_buffer, INDIRECT_ZONE);
            for i in 0..PTR_INDEX_MAX {
                if rs.izone_present(i) {
                    if rs.in_window() {
                        Self::read_indirect_data(rs.izones, i, buffer, rs);
                        if rs.bytes_left == 0 {
                            return rs.bytes_read;
                        }
                    }
                    rs.seen_block()
                }
            }
        }
        0
    }

    fn double_indirect_zones(inode: &Inode, buffer: *mut u8, rs: &mut ReadState) -> u32 {
        if inode.zones[DOUBLE_INDIRECT_ZONE] != 0 {
            Self::read_zone(inode, &mut rs.indirect_buffer, DOUBLE_INDIRECT_ZONE);
            for i in 0..PTR_INDEX_MAX {
                if rs.izone_present(i) {
                    Self::read_izone(rs.izones, &mut rs.double_indirect_buffer, i);
                    for j in 0..PTR_INDEX_MAX {
                        if rs.iizone_present(j) {
                            if rs.in_window() {
                                Self::read_indirect_data(rs.iizones, j, buffer, rs);
                                if rs.bytes_left == 0 {
                                    return rs.bytes_read;
                                }
                            }
                            rs.seen_block()
                        }
                    }
                }
            }
        }
        0
    }

    fn triple_indirect_zones(inode: &Inode, buffer: *mut u8, rs: &mut ReadState) -> u32 {
        if inode.zones[TRIPLE_INDIRECT_ZONE] != 0 {
            Self::read_zone(inode, &mut rs.indirect_buffer, TRIPLE_INDIRECT_ZONE);
            for i in 0..PTR_INDEX_MAX {
                if rs.izone_present(i) {
                    Self::read_izone(rs.izones, &mut rs.double_indirect_buffer, i);
                    for j in 0..PTR_INDEX_MAX {
                        if rs.iizone_present(j) {
                            Self::read_izone(rs.iizones, &mut rs.triple_indirect_buffer, j);
                            for k in 0..PTR_INDEX_MAX {
                                if rs.iiizone_present(k) {
                                    if rs.in_window() {
                                        Self::read_indirect_data(rs.iiizones, k, buffer, rs);
                                        if rs.bytes_left == 0 {
                                            return rs.bytes_read;
                                        }
                                    }
                                    rs.seen_block()
                                }
                            }
                        }
                    }
                }
            }
        }
        0
    }

    pub fn read(inode: &Inode, buffer: *mut u8, size: u32, offset: u32) -> u32 {
        let mut rs = ReadState::new(inode.size, size, offset);

        let br = Self::direct_zones(inode, buffer, &mut rs);
        if br != 0 {
            return br;
        }

        let br = Self::indirect_zones(inode, buffer, &mut rs);
        if br != 0 {
            return br;
        }

        let br = Self::double_indirect_zones(inode, buffer, &mut rs);
        if br != 0 {
            return br;
        }

        let br = Self::triple_indirect_zones(inode, buffer, &mut rs);
        if br != 0 {
            return br;
        }

        rs.bytes_read
    }

    pub fn read_file(file_name: &str, buffer: *mut u8, size: u32, offset: u32) -> u32 {
        if let Some(node) = unsafe { MFS_INODE_CACHE.get(file_name) } {
            Self::read(node, buffer, size, offset)
        } else {
            println!("Unable to find '{}' in MFS_INODE_CACHE", file_name);
            0
        }
    }

    #[allow(dead_code)]
    pub fn write(&mut self, _desc: &Inode, _buffer: *const u8, _offset: u32, _size: u32) -> u32 {
        todo!();
    }
}

pub fn init() {
    MinixFileSystem::init();
}

pub fn debug_cache() {
    serial_debug("FS Cache");
    for (strg, node) in unsafe { MFS_INODE_CACHE.iter() } {
        println!("{}: {:?}", strg, node);
    }
}
