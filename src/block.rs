use crate::alloc::{alloc_bytes, alloc_pages_zeroed, free_bytes};
use crate::assembly;
use crate::config::{PAGE_SIZE, WAIT_FOR_READY};
use crate::uart::serial_info;
use crate::{print, println};
use core::mem::size_of;

// mod block.rs
// This is an extremely simple block driver using virtio legacy mmio

// Static handle for default configured block device
static mut BLOCK_DEVICE: Option<BlockDevice> = None;

const MMIO_HOST_FEATURES: usize = 0x010 / 4;
const MMIO_GUEST_FEATURES: usize = 0x020 / 4;
const MMIO_GUEST_PAGE_SIZE: usize = 0x028 / 4;
const MMIO_QUEUE_SELECT: usize = 0x030 / 4;
const MMIO_QUEUE_NUMBER_MAX: usize = 0x034 / 4;
const MMIO_QUEUE_NUMBER: usize = 0x038 / 4;
const MMIO_QUEUE_PFN: usize = 0x040 / 4;
const MMIO_QUEUE_NOTIFY: usize = 0x050 / 4;
const MMIO_STATUS: usize = 0x070 / 4;

const VIRTIO_DESC_FLAG_NEXT: u16 = 1;
const VIRTIO_DESC_FLAG_WRITE: u16 = 2;

const VIRTIO_BLK_TYPE_IN: u32 = 0;
const VIRTIO_BLK_TYPE_OUT: u32 = 1;

const STATUS_FIELD_ACKNOWLEDGE: u32 = 1;
const STATUS_FIELD_DRIVER_OK: u32 = 4;
const STATUS_FIELD_FEATURES_OK: u32 = 8;
const STATUS_FIELD_FAILED: u32 = 128;

const VIRTIO_FEATURE_RO: u32 = 1 << 5;
const VIRTIO_RING_SIZE: usize = 1 << 7;

const READ: bool = false;
const WRITE: bool = true;

#[repr(C)]
pub struct Header {
    blktype: u32,
    reserved: u32,
    sector: u64,
}

#[repr(C)]
pub struct Data {
    data: *mut u8,
}

#[repr(C)]
pub struct Status {
    status: u8,
}

#[repr(C)]
pub struct Request {
    header: Header,
    data: Data,
    status: Status,
    head: u16,
    watcher: u16,
}

#[repr(C)]
pub struct Descriptor {
    pub addr: u64,
    pub len: u32,
    pub flags: u16,
    pub next: u16,
}

#[repr(C)]
pub struct Available {
    pub flags: u16,
    pub idx: u16,
    pub ring: [u16; VIRTIO_RING_SIZE],
    pub event: u16,
}

#[repr(C)]
pub struct UsedElem {
    pub id: u32,
    pub len: u32,
}

#[repr(C)]
pub struct Used {
    pub flags: u16,
    pub idx: u16,
    pub ring: [UsedElem; VIRTIO_RING_SIZE],
    pub event: u16,
}

#[repr(C)]
pub struct Queue {
    pub desc: [Descriptor; VIRTIO_RING_SIZE],
    pub avail: Available,
    pub padding0:
        [u8; PAGE_SIZE - size_of::<Descriptor>() * VIRTIO_RING_SIZE - size_of::<Available>()],
    pub used: Used,
}

pub struct BlockDevice {
    queue: *mut Queue,
    dev: *mut u32,
    idx: u16,
    ack_used_idx: u16,
    read_only: bool,
    ready: [bool; VIRTIO_RING_SIZE],
}

impl BlockDevice {
    unsafe fn init_status(ptr: *mut u32) -> u32 {
        ptr.add(MMIO_STATUS).write_volatile(0);

        let mut status_bits = STATUS_FIELD_ACKNOWLEDGE;
        ptr.add(MMIO_STATUS).write_volatile(status_bits);

        status_bits |= STATUS_FIELD_DRIVER_OK;
        ptr.add(MMIO_STATUS).write_volatile(status_bits);
        status_bits
    }

    unsafe fn init_guest_features(ptr: *mut u32) -> bool {
        let host_features = ptr.add(MMIO_HOST_FEATURES).read_volatile();
        let guest_features = host_features & !(VIRTIO_FEATURE_RO);
        ptr.add(MMIO_GUEST_FEATURES).write_volatile(guest_features);
        host_features & (VIRTIO_FEATURE_RO) != 0
    }

    unsafe fn init_status_check(ptr: *mut u32, status_bits: u32) -> (bool, u32) {
        let sb_out = status_bits | STATUS_FIELD_FEATURES_OK;
        ptr.add(MMIO_STATUS).write_volatile(sb_out);

        let status_ok = ptr.add(MMIO_STATUS).read_volatile();
        if (status_ok & STATUS_FIELD_FEATURES_OK) == 0 {
            print!("features fail...");
            ptr.add(MMIO_STATUS).write_volatile(STATUS_FIELD_FAILED);
            return (false, 0);
        }
        (true, sb_out)
    }

    unsafe fn init_queue_check(ptr: *mut u32) -> bool {
        let qnmax = ptr.add(MMIO_QUEUE_NUMBER_MAX).read_volatile();
        if VIRTIO_RING_SIZE > qnmax.try_into().unwrap() {
            print!("queue size fail...");
            return false;
        }
        ptr.add(MMIO_QUEUE_NUMBER)
            .write_volatile(VIRTIO_RING_SIZE.try_into().unwrap());
        ptr.add(MMIO_QUEUE_SELECT).write_volatile(0);
        true
    }

    unsafe fn init_pfn(ptr: *mut u32) -> *mut Queue {
        let num_pages = (size_of::<Queue>() + PAGE_SIZE - 1) / PAGE_SIZE;
        let queue_ptr = alloc_pages_zeroed(num_pages) as *mut Queue;
        let queue_pfn = queue_ptr as u32;
        ptr.add(MMIO_GUEST_PAGE_SIZE)
            .write_volatile(PAGE_SIZE.try_into().unwrap());
        ptr.add(MMIO_QUEUE_PFN)
            .write_volatile(queue_pfn / PAGE_SIZE as u32);
        queue_ptr
    }

    unsafe fn init_bd(ptr: *mut u32, queue_ptr: *mut Queue, ro: bool) {
        let bd = BlockDevice {
            queue: queue_ptr,
            dev: ptr,
            idx: 0,
            ack_used_idx: 0,
            read_only: ro,
            ready: [true; VIRTIO_RING_SIZE],
        };
        BLOCK_DEVICE = Some(bd);
    }

    unsafe fn init_notify(ptr: *mut u32, status_bits: u32) -> bool {
        ptr.add(MMIO_STATUS)
            .write_volatile(status_bits | STATUS_FIELD_DRIVER_OK);
        true
    }

    fn init(ptr: *mut u32) -> bool {
        serial_info("init block device");
        unsafe {
            let status_bits = BlockDevice::init_status(ptr);
            let ro = BlockDevice::init_guest_features(ptr);

            let (pass, status_bits) = BlockDevice::init_status_check(ptr, status_bits);
            if !pass {
                return false;
            }

            if !BlockDevice::init_queue_check(ptr) {
                return false;
            }

            BlockDevice::init_bd(ptr, BlockDevice::init_pfn(ptr), ro);

            BlockDevice::init_notify(ptr, status_bits)
        }
    }

    unsafe fn use_queue(&mut self) {
        let queue = &(*self.queue);
        while self.ack_used_idx != queue.used.idx {
            let idx = self.ack_used_idx as usize % VIRTIO_RING_SIZE;
            let elem = &queue.used.ring[idx];
            self.ack_used_idx = self.ack_used_idx.wrapping_add(1);
            self.ready[idx] = true;
            let rq = queue.desc[elem.id as usize].addr as *const Request;
            free_bytes(rq as *mut u8);
        }
    }

    unsafe fn block_header(
        &mut self,
        buffer: *mut u8,
        offset: u64,
        write: bool,
    ) -> (*mut Request, u16) {
        let sector = offset / 512;
        let blk_request_size = size_of::<Request>();
        let blk_request = alloc_bytes(blk_request_size) as *mut Request;
        let desc = Descriptor {
            addr: &(*blk_request).header as *const Header as u64,
            len: size_of::<Header>() as u32,
            flags: VIRTIO_DESC_FLAG_NEXT,
            next: 0,
        };
        let head_idx = self.fill_next_descriptor(desc);
        (*blk_request).header.sector = sector;
        (*blk_request).header.blktype = if write {
            VIRTIO_BLK_TYPE_OUT
        } else {
            VIRTIO_BLK_TYPE_IN
        };
        (*blk_request).data.data = buffer;
        (*blk_request).header.reserved = 0;
        (*blk_request).status.status = 111;
        (blk_request, head_idx)
    }

    unsafe fn block_data(&mut self, buffer: *mut u8, size: u32, write: bool) {
        let desc = Descriptor {
            addr: buffer as u64,
            len: size,
            flags: VIRTIO_DESC_FLAG_NEXT | if !write { VIRTIO_DESC_FLAG_WRITE } else { 0 },
            next: 0,
        };
        let _data_idx = self.fill_next_descriptor(desc);
    }

    unsafe fn block_status(&mut self, blk_request: *mut Request) {
        let desc = Descriptor {
            addr: &(*blk_request).status as *const Status as u64,
            len: size_of::<Status>() as u32,
            flags: VIRTIO_DESC_FLAG_WRITE,
            next: 0,
        };
        let _status_idx = self.fill_next_descriptor(desc);
    }

    unsafe fn block_notify(&mut self, head_idx: u16) -> usize {
        let idx = (*self.queue).avail.idx as usize % VIRTIO_RING_SIZE;
        (*self.queue).avail.ring[idx] = head_idx;
        (*self.queue).avail.idx = (*self.queue).avail.idx.wrapping_add(1);
        self.ready[idx] = false;
        self.dev.add(MMIO_QUEUE_NOTIFY).write_volatile(0);
        idx
    }

    unsafe fn block_operation(&mut self, buffer: *mut u8, size: u32, offset: u64, write: bool) {
        if self.read_only && write {
            println!("Trying to write to read/only!");
            return;
        }
        let (blk_request, head_idx) = self.block_header(buffer, offset, write);
        self.block_data(buffer, size, write);
        self.block_status(blk_request);
        let idx = self.block_notify(head_idx);
        let mut counter = 0;
        while counter < WAIT_FOR_READY && !self.ready[idx] {
            assembly::no_operation();
            counter += 1;
        }
    }

    unsafe fn fill_next_descriptor(&mut self, desc: Descriptor) -> u16 {
        self.idx = (self.idx + 1) % VIRTIO_RING_SIZE as u16;
        (*self.queue).desc[self.idx as usize] = desc;
        if (*self.queue).desc[self.idx as usize].flags & VIRTIO_DESC_FLAG_NEXT != 0 {
            (*self.queue).desc[self.idx as usize].next = (self.idx + 1) % VIRTIO_RING_SIZE as u16;
        }
        self.idx
    }
}

// ====================================================
// The public interface for the block device is here...
// ====================================================

// init must be called once to enable the block read, write,
// and interrupt API. It is called by virtio::init() when
// initializing the default block device
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub fn init(ptr: *mut u32) -> bool {
    BlockDevice::init(ptr)
}

// The block device specific logic for virtio interrupt handling
// Called from virtio::interrupt_handler() for device 8
// which is the default block device interrupt
pub fn interrupt_handler() {
    unsafe {
        if let Some(bdev) = BLOCK_DEVICE.as_mut() {
            bdev.use_queue();
        } else {
            println!("Unable to retrieve default block device");
        }
    }
}

// Read data from disk device to buffer
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub fn read(buffer: *mut u8, size: u32, offset: u64) {
    unsafe {
        if let Some(bdev) = BLOCK_DEVICE.as_mut() {
            bdev.block_operation(buffer, size, offset, READ);
        } else {
            println!("Unable to retrieve default block device");
        }
    }
}

// Write data from buffer to disk device
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub fn write(buffer: *mut u8, size: u32, offset: u64) {
    unsafe {
        if let Some(bdev) = BLOCK_DEVICE.as_mut() {
            bdev.block_operation(buffer, size, offset, WRITE);
        } else {
            println!("Unable to retrieve default block device");
        }
    }
}
