use crate::config::PAGE_SIZE;
use crate::debug;
use crate::memory::align_val;
use crate::uart::serial_info;
use crate::{print, println};
use core::{mem::size_of, ptr::null_mut};

// mod alloc.rs
// This is the kernel page and byte grain heap allocators

// Symbols defined in cfg/link.ld & asm/layout.S
extern "C" {
    static HEAP_START: usize;
    static HEAP_SIZE: usize;
    static MEMORY_END: usize;
}

const PAGE_ORDER: usize = 12;
const ALLOC_TAKEN: usize = 1 << 63;

const PAGE_FLAG_EMPTY: u8 = 0;
const PAGE_FLAG_TAKEN: u8 = 1;
const PAGE_FLAG_LAST: u8 = 2;

// This is the PageGrainAllocator state
static mut PAGE_GRAIN_ALLOC: PageGrainAllocator = PageGrainAllocator {};

struct PageGrainAllocator {}

impl PageGrainAllocator {
    fn init() {
        serial_info("init kernel memory allocator");
        unsafe {
            let num_pages = HEAP_SIZE / PAGE_SIZE;
            let ptr = HEAP_START as *mut PageGrainFlags;
            for i in 0..num_pages {
                (*ptr.add(i)).clear();
            }
        }
    }

    fn alloc(&self, pages: usize) -> *mut u8 {
        assert!(pages > 0);
        unsafe {
            let num_pages = HEAP_SIZE / PAGE_SIZE;
            let ptr = HEAP_START as *mut PageGrainFlags;
            for i in 0..=num_pages - pages {
                let mut found = false;
                if (*ptr.add(i)).is_free() {
                    found = true;
                    for j in i..i + pages {
                        if (*ptr.add(j)).is_taken() {
                            found = false;
                            break;
                        }
                    }
                }
                if found {
                    for k in i..=i + pages - 1 {
                        (*ptr.add(k)).set_flag(PAGE_FLAG_TAKEN);
                    }
                    (*ptr.add(i + pages - 1)).set_flag(PAGE_FLAG_LAST);
                    return (BYTE_GRAIN_ALLOC.get_start() + PAGE_SIZE * i) as *mut u8;
                }
            }
        }
        null_mut()
    }

    fn zalloc(&self, pages: usize) -> *mut u8 {
        let ret = alloc_pages(pages);
        if !ret.is_null() {
            let size = (PAGE_SIZE * pages) / 8;
            let big_ptr = ret as *mut u64;
            for i in 0..size {
                unsafe {
                    (*big_ptr.add(i)) = 0;
                }
            }
        }
        ret
    }

    fn print(&self) {
        unsafe {
            let num_pages = HEAP_SIZE / PAGE_SIZE;
            let mut beg = HEAP_START as *const PageGrainFlags;
            let end = beg.add(num_pages);
            let alloc_beg = BYTE_GRAIN_ALLOC.get_start();
            let alloc_end = MEMORY_END;
            let avail_pages = (alloc_end - alloc_beg) / 4096;
            debug::dbg(
                "Kernel Allocator Memory Map\n\nRANGE:       START         END           PAGES",
            );
            println!(
                "- METADATA:  {:p} -> {:p}: {:>7} \n\
						 - PAGES:     0x{:x} -> 0x{:x}: {:>7}",
                beg,
                end,
                (alloc_beg - HEAP_START) / 4096,
                alloc_beg,
                alloc_end,
                avail_pages
            );
            println!("\nPage Grain Allocator");
            println!("----------------------------------------------");
            let mut num = 0;
            while beg < end {
                if (*beg).is_taken() {
                    let start = beg as usize;
                    let memaddr = BYTE_GRAIN_ALLOC.get_start() + (start - HEAP_START) * PAGE_SIZE;
                    let name = if memaddr as *mut ByteGrainFlags == BYTE_GRAIN_ALLOC.get_head() {
                        "BGA"
                    } else {
                        "   "
                    };
                    print!("- {}        0x{:x} => ", name, memaddr);
                    loop {
                        num += 1;
                        if (*beg).is_last() {
                            let end = beg as usize;
                            let memaddr = BYTE_GRAIN_ALLOC.get_start()
                                + (end - HEAP_START) * PAGE_SIZE
                                + PAGE_SIZE
                                - 1;
                            print!("0x{:x}: {:>7}", memaddr, (end - start + 1));
                            println!("");
                            break;
                        }
                        beg = beg.add(1);
                    }
                }
                beg = beg.add(1);
            }
            println!("----------------------------------------------");
            println!(
                "Allocated: {:>6}/{:>6} pages {}%",
                num,
                avail_pages,
                num * 100 / avail_pages
            );
        }
    }
}

// This is the ByteGrainAllocator state
static mut BYTE_GRAIN_ALLOC: ByteGrainAllocator = ByteGrainAllocator {
    head: null_mut(),
    alloc: 0,
    start: 0,
};

struct ByteGrainAllocator {
    head: *mut ByteGrainFlags,
    alloc: usize,
    start: usize,
}

impl ByteGrainAllocator {
    fn get_head(&self) -> *mut ByteGrainFlags {
        self.head
    }

    fn get_head_u8(&self) -> *mut u8 {
        self.head as *mut u8
    }

    fn get_alloc(&self) -> usize {
        self.alloc
    }

    fn get_start(&self) -> usize {
        self.start
    }

    fn set_head(&mut self, head: *mut ByteGrainFlags) {
        self.head = head;
    }

    fn set_alloc(&mut self, alloc: usize) {
        self.alloc = alloc;
    }

    fn set_start(&mut self, start: usize) {
        self.start = start;
    }

    fn init() {
        unsafe {
            let num_pages = HEAP_SIZE / PAGE_SIZE;
            BYTE_GRAIN_ALLOC.set_start(align_val(
                HEAP_START + num_pages * size_of::<PageGrainFlags>(),
                PAGE_ORDER,
            ));
            BYTE_GRAIN_ALLOC.set_alloc(512);
            let k_alloc = alloc_pages_zeroed(BYTE_GRAIN_ALLOC.get_alloc());
            assert!(!k_alloc.is_null());
            BYTE_GRAIN_ALLOC.set_head(k_alloc as *mut ByteGrainFlags);
            (*BYTE_GRAIN_ALLOC.get_head()).set_free();
            (*BYTE_GRAIN_ALLOC.get_head()).set_size(BYTE_GRAIN_ALLOC.get_alloc() * PAGE_SIZE);
        }
    }

    fn kzmalloc(&mut self, sz: usize) -> *mut u8 {
        let size = align_val(sz, 3);
        let ret = self.kmalloc(size);

        if !ret.is_null() {
            for i in 0..size {
                unsafe {
                    (*ret.add(i)) = 0;
                }
            }
        }
        ret
    }

    fn kmalloc(&mut self, sz: usize) -> *mut u8 {
        unsafe {
            let size = align_val(sz, 3) + size_of::<ByteGrainFlags>();
            let mut head = self.get_head();
            let tail = self.get_head_u8().add(self.get_alloc() * PAGE_SIZE) as *mut ByteGrainFlags;

            while head < tail {
                if (*head).is_free() && size <= (*head).get_size() {
                    let chunk_size = (*head).get_size();
                    let rem = chunk_size - size;
                    (*head).set_taken();
                    if rem > size_of::<ByteGrainFlags>() {
                        let next = (head as *mut u8).add(size) as *mut ByteGrainFlags;
                        (*next).set_free();
                        (*next).set_size(rem);
                        (*head).set_size(size);
                    } else {
                        (*head).set_size(chunk_size);
                    }
                    return head.add(1) as *mut u8;
                } else {
                    head = (head as *mut u8).add((*head).get_size()) as *mut ByteGrainFlags;
                }
            }
        }
        null_mut()
    }

    fn kfree(&mut self, ptr: *mut u8) {
        unsafe {
            if !ptr.is_null() {
                let p = (ptr as *mut ByteGrainFlags).offset(-1);
                if (*p).is_taken() {
                    (*p).set_free();
                }
                self.coalesce();
            }
        }
    }

    #[allow(dead_code)]
    fn coalesce(&mut self) {
        unsafe {
            let mut head = self.get_head();
            let tail = self.get_head_u8().add(self.get_alloc() * PAGE_SIZE) as *mut ByteGrainFlags;

            while head < tail {
                let next = (head as *mut u8).add((*head).get_size()) as *mut ByteGrainFlags;
                if (*head).get_size() == 0 || next >= tail {
                    break;
                } else if (*head).is_free() && (*next).is_free() {
                    (*head).set_size((*head).get_size() + (*next).get_size());
                }
                head = (head as *mut u8).add((*head).get_size()) as *mut ByteGrainFlags;
            }
        }
    }

    fn print(&self) {
        unsafe {
            println!("\nByte Grain Allocator (BGA)               BYTES");
            println!("----------------------------------------------");
            let mut head = self.get_head();
            let tail = self.get_head_u8().add(self.get_alloc() * PAGE_SIZE) as *mut ByteGrainFlags;
            let mut total_bytes = 0;
            let mut used_bytes = 0;
            while head < tail {
                total_bytes += (*head).get_size();
                if (*head).is_taken() {
                    used_bytes += (*head).get_size()
                }
                println!(
                    "- {}      {:p} => {:p}: {:>7}",
                    if (*head).is_taken() { "TAKEN" } else { "     " },
                    head,
                    (head as *mut u8).add((*head).get_size()),
                    (head as *mut u8)
                        .add((*head).get_size())
                        .offset_from(head as *mut u8)
                );
                head = (head as *mut u8).add((*head).get_size()) as *mut ByteGrainFlags;
            }
            println!("----------------------------------------------");
            println!(
                "Allocated: {:>6}/{:>6} bytes {}%\n",
                used_bytes,
                total_bytes,
                used_bytes / total_bytes
            );
        }
    }
}

// This structure is used to track byte grain allocations within a page grained allocation
struct ByteGrainFlags {
    flags: usize,
}
impl ByteGrainFlags {
    fn is_taken(&self) -> bool {
        self.flags & ALLOC_TAKEN != 0
    }

    fn is_free(&self) -> bool {
        !self.is_taken()
    }

    fn set_taken(&mut self) {
        self.flags |= ALLOC_TAKEN;
    }

    fn set_free(&mut self) {
        self.flags &= !ALLOC_TAKEN;
    }

    fn set_size(&mut self, sz: usize) {
        let k = self.is_taken();
        self.flags = sz & !ALLOC_TAKEN;
        if k {
            self.flags |= ALLOC_TAKEN;
        }
    }

    fn get_size(&self) -> usize {
        self.flags & !ALLOC_TAKEN
    }
}

// This structure tracks page grained allocations
struct PageGrainFlags {
    flags: u8,
}

impl PageGrainFlags {
    fn is_last(&self) -> bool {
        self.flags & PAGE_FLAG_LAST != 0
    }

    fn is_taken(&self) -> bool {
        self.flags & PAGE_FLAG_TAKEN != 0
    }

    fn is_free(&self) -> bool {
        !self.is_taken()
    }

    fn clear(&mut self) {
        self.flags = PAGE_FLAG_EMPTY;
    }

    fn set_flag(&mut self, flag: u8) {
        self.flags |= flag;
    }
}

// Beginning of public alloc API
pub fn init() {
    PageGrainAllocator::init();
    ByteGrainAllocator::init();
}

// Allocate kernel memory pages
pub fn alloc_pages(pages: usize) -> *mut u8 {
    unsafe { PAGE_GRAIN_ALLOC.alloc(pages) }
}

// Allocate zeroed kernel memory pages
pub fn alloc_pages_zeroed(pages: usize) -> *mut u8 {
    unsafe { PAGE_GRAIN_ALLOC.zalloc(pages) }
}

// Allocate zeroed bytes from kernel byte allocator
pub fn alloc_bytes_zeroed(sz: usize) -> *mut u8 {
    unsafe { BYTE_GRAIN_ALLOC.kzmalloc(sz) }
}

// Allocate bytes from kernel byte allocator
pub fn alloc_bytes(sz: usize) -> *mut u8 {
    unsafe { BYTE_GRAIN_ALLOC.kmalloc(sz) }
}

// Free bytes from kernel byte allocator
pub fn free_bytes(ptr: *mut u8) {
    unsafe { BYTE_GRAIN_ALLOC.kfree(ptr) };
}

// Helpful debugging aid to visualize kernel memory heap
pub fn debug_heap() {
    unsafe {
        PAGE_GRAIN_ALLOC.print();
        BYTE_GRAIN_ALLOC.print();
    }
}

use core::alloc::{GlobalAlloc, Layout};
struct OsGlobalAlloc;
unsafe impl GlobalAlloc for OsGlobalAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        alloc_bytes_zeroed(layout.size())
    }

    unsafe fn dealloc(&self, ptr: *mut u8, _layout: Layout) {
        free_bytes(ptr);
    }
}

#[global_allocator]
static GA: OsGlobalAlloc = OsGlobalAlloc {};

#[alloc_error_handler]
pub fn alloc_error(l: Layout) -> ! {
    panic!(
        "Kernel page allocator failed to allocate {} bytes with {}-byte alignment.",
        l.size(),
        l.align()
    );
}
