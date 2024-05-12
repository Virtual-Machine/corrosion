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
static mut MFS_SUPERBLOCK_CACHE: SuperBlock = SuperBlock {
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
        unsafe { MFS_SUPERBLOCK_CACHE.get_inode(inode_num) }
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
        unsafe { MFS_SUPERBLOCK_CACHE = *super_block };
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

fn bit_count(byte: u8) -> u32 {
    match byte {
        0 => 0,
        1 | 2 | 4 | 8 | 16 | 32 | 64 | 128 => 1,
        3 | 5 | 6 | 9 | 10 | 12 | 17 | 18 | 20 | 24 | 33 | 34 | 36 | 40 | 48 | 65 | 66 | 68 | 72 | 80 | 96 | 129 | 130 | 132 | 136 | 144 | 160 | 192 => 2,
        7 | 11 | 13 | 14 | 19 | 21 | 22 | 25 | 26 | 28 | 35 | 37 | 38 | 41 | 42 | 44 | 49 | 50 | 52 | 56 | 67 | 69 | 70 | 73 | 74 | 76 | 81 | 82 | 84 | 88 | 97 | 98 | 100 | 104 | 112 | 131 | 133 | 134 | 137 | 138 | 140 | 145 | 146 | 148 | 152 | 161 | 162 | 164 | 168 | 176 | 193 | 194 | 196 | 200 | 208 | 224 => 3,
        15 | 23 | 27 | 29 | 30 | 39 | 43 | 45 | 46 | 51 | 53 | 54 | 57 | 58 | 60 | 71 | 75 | 77 | 78 | 83 | 85 | 86 | 89 | 90 | 92 | 99 | 101 | 102 | 105 | 106 | 108 | 113 | 114 | 116 | 120 | 135 | 139 | 141 | 142 | 147 | 149 | 150 | 153 | 154 | 156 | 163 | 165 | 166 | 169 | 170 | 172 | 177 | 178 | 180 | 184 | 195 | 197 | 198 | 201 | 202 | 204 | 209 | 210 | 212 | 216 | 225 | 226 | 228 | 232 | 240 => 4,
        31 | 47 | 55 | 59 | 61 | 62 | 79 | 87 | 91 | 93 | 94 | 103 | 107 | 109 | 110 | 115 | 117 | 118 | 121 | 122 | 124 | 143 | 151 | 155 | 157 | 158 | 167 | 171 | 173 | 174 | 179 | 181 | 182 | 185 | 186 | 188 | 199 | 203 | 205 | 206 | 211 | 213 | 214 | 217 | 218 | 220 | 227 | 229 | 230 | 233 | 234 | 236 | 241 | 242 | 244 | 248 => 5,
        63 | 95 | 111 | 119 | 123 | 125 | 126 | 159 | 175 | 183 | 187 | 189 | 190 | 207 | 215 | 219 | 221 | 222 | 231 | 235 | 237 | 238 | 243 | 245 | 246 | 249 | 250 | 252 => 6,
        127 | 191 | 223 | 239 | 247 | 251 | 253 | 254 => 7,
        255 => 8,
    }
}

fn print_bitmap(read_size: u32, offset: u64, items: u32) -> u32 {
    let mut buffer = Buffer::new(read_size as usize);
    block::read(buffer.get_mut(), read_size, offset);
    let mut previous_print = true;
    let mut total_bit_count = 0;
    for i in 0..items {
        let val = unsafe { buffer.get().add(i as usize).read()};
        total_bit_count += bit_count(val);
        // Print first, last, and non 0 bytes
        if i == 0 || i == items - 1 || val != 0x0 {
            print!("{:08b} ", val);
            previous_print = true;
        } else {
            if previous_print {
                // If the previous byte was printed show ellipsis 
                // to indicate a skipped range of byte(s)
                print!("........ ");
                previous_print = false;
            }
        }
    };
    total_bit_count
}

fn find_first_free_inode() {
    let read_size = BLOCK_SIZE * unsafe{MFS_SUPERBLOCK_CACHE}.imap_blocks as u32;
    let offset = (BLOCK_SIZE * 2) as u64;
    let mut buffer = Buffer::new(read_size as usize);
    block::read(buffer.get_mut(), read_size, offset);
    for byte_idx in 0..unsafe{MFS_SUPERBLOCK_CACHE}.ninodes/8 {
        let byte = unsafe { buffer.get().add(byte_idx as usize).read()};
        if byte != 0xff {
            for bit_idx in 0..8 {
                if (byte & (1 << bit_idx)) == 0 {
                    let inode_idx = (byte_idx * 8 + bit_idx) as u32;
                    println!("First available inode: {}", (inode_idx + 1));
                    return;
                }
            }
        }
    }
    println!("No available inode found!");
}

fn find_first_free_zone() {
    let read_size = BLOCK_SIZE * unsafe{MFS_SUPERBLOCK_CACHE}.zmap_blocks as u32;
    let offset = (BLOCK_SIZE * (2 + unsafe{MFS_SUPERBLOCK_CACHE}.imap_blocks as u32)) as u64;
    let mut buffer = Buffer::new(read_size as usize);
    block::read(buffer.get_mut(), read_size, offset);
    for byte_idx in 0..unsafe{MFS_SUPERBLOCK_CACHE}.zones/8 {
        let byte = unsafe { buffer.get().add(byte_idx as usize).read()};
        if byte != 0xff {
            for bit_idx in 0..8 {
                if (byte & (1 << bit_idx)) == 0 {
                    let inode_idx = (byte_idx * 8 + bit_idx) as u32;
                    println!("First available zone: {}", (inode_idx + 1));
                    return;
                }
            }
        }
    }
    println!("No available zone found!");
}

pub fn debug_fs() {
    let superblock_cache = unsafe{MFS_SUPERBLOCK_CACHE};
    serial_debug("FS");
    println!("SuperBlock:");
    println!("  # of inodes    : {}", superblock_cache.ninodes);
    println!("  padding 0      : {}", superblock_cache.pad0);
    println!("  inode blocks   : {}", superblock_cache.imap_blocks);
    println!("  zone blocks    : {}", superblock_cache.zmap_blocks);
    println!("  first data zone: {}", superblock_cache.first_data_zone);
    println!("  log zone size  : {}", superblock_cache.log_zone_size);
    println!("  padding 1      : {}", superblock_cache.pad1);
    println!("  max size       : {}", superblock_cache.max_size);
    println!("  zones          : {}", superblock_cache.zones);
    println!("  magic          : {}", superblock_cache.magic);
    println!("  padding 2      : {}", superblock_cache.pad2);
    println!("  block size     : {}", superblock_cache.block_size);
    println!("  disk version   : {}", superblock_cache.disk_version);

    let inodes = superblock_cache.ninodes;
    let zones = superblock_cache.zones;
    let imap_blocks = superblock_cache.imap_blocks as u32;
    let zmap_blocks = superblock_cache.zmap_blocks as u32;
    let first_data_zone = superblock_cache.first_data_zone as u32;

    println!("\nInode Bitmap:");
    let read_size = BLOCK_SIZE * imap_blocks;
    let offset = (BLOCK_SIZE * 2) as u64;
    let count = print_bitmap(read_size, offset, inodes/8);
    println!("\n  Used {} / {} inodes ({}%)", count, inodes, count * 100 / inodes);

    find_first_free_inode();

    println!("\nZone Bitmap:");
    let read_size = BLOCK_SIZE * zmap_blocks;
    let offset = (BLOCK_SIZE * (2 + imap_blocks)) as u64;
    let count = print_bitmap(read_size, offset, zones/8 - first_data_zone);    
    println!("\n  Used {} / {} zones ({}%)", count, zones, count * 100 / zones);

    find_first_free_zone();

    // Print the inode representing the root directory
    if let Some(node) = superblock_cache.get_inode(1){
        println!("{:?}", node);
    }

    // Print the test file inside the root directory
    if let Some(node) = superblock_cache.get_inode(2){
        println!("{:?}", node);
    }
}
