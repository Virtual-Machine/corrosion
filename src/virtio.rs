use crate::block;
use crate::uart::serial_info;
use crate::{print, println};

// mod virtio.rs
// A simple driver for interacting with legacy MMIO devices in QEMU

const VIRTIO_START: usize = 0x1000_1000; // address of first virtio device
const VIRTIO_END: usize = 0x1000_8000; // address of last virtio device
const VIRTIO_STRIDE: usize = 0x1000; // step by 4k per device
const VIRTIO_MAGIC_LE: u32 = 0x74_72_69_76; // 'VIRT' in little endian ascii

// const NETWORK: u32 = 1;
const BLOCK: u32 = 2;
// const RANDOM: u32 = 4;
const GPU: u32 = 16;
const INPUT: u32 = 18;

static mut VIRTIO_DEVICE_TYPES: [Option<u32>; 8] = [None, None, None, None, None, None, None, None];

fn set_virtio_device_type(addr: usize, value: u32) {
    let idx = (addr - VIRTIO_START) >> 12;
    unsafe {
        VIRTIO_DEVICE_TYPES[idx] = Some(value);
    }
}

pub fn init() {
    serial_info("init virtio");
    for addr in (VIRTIO_START..=VIRTIO_END).step_by(VIRTIO_STRIDE) {
        print!("    - Virtio device @ 0x{:08x}...", addr);
        let magicvalue;
        let deviceid;
        let ptr = addr as *mut u32;
        unsafe {
            magicvalue = ptr.read_volatile();
            deviceid = ptr.add(2).read_volatile();
        }
        if VIRTIO_MAGIC_LE != magicvalue {
            println!("...not virtio.");
        } else if 0 == deviceid {
            println!("...not connected.");
        } else {
            match deviceid {
                BLOCK => {
                    if !block::init(ptr) {
                        println!("failed to init block device...");
                        continue;
                    }
                    set_virtio_device_type(addr, BLOCK);
                }
                GPU => {
                    println!("GPU device...");
                    set_virtio_device_type(addr, GPU);
                }
                INPUT => {
                    println!("input device...");
                    set_virtio_device_type(addr, INPUT);
                }
                _ => println!("...ignored device type {}.", deviceid),
            }
        }
    }
}

pub fn interrupt_handler(interrupt: u32) {
    let idx = interrupt as usize - 1;
    unsafe {
        if let Some(vd) = &VIRTIO_DEVICE_TYPES[idx] {
            match *vd {
                BLOCK => {
                    block::interrupt_handler();
                }
                _ => {
                    println!("Invalid device generated interrupt: {}!", vd);
                }
            }
        } else {
            println!("Spurious interrupt {}", interrupt);
        }
    }
}
