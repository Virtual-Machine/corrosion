use crate::print;
use crate::println;
use crate::uart::serial_info;
use crate::virtio;

// mod plic.rs
// This is a very simple PLIC driver that enables 8 PLIC interrupts
// @ priority 1 / threshold @ 0.

const PLIC_PRIORITY: usize = 0x0C00_0000;
const PLIC_INT_ENABLE: usize = 0x0C00_2000;
const PLIC_THRESHOLD: usize = 0x0C20_0000;
const PLIC_CLAIM: usize = 0x0C20_0004;

fn next_plic_interrupt() -> Option<u32> {
    let claim_register = PLIC_CLAIM as *const u32;
    let claim_number;
    unsafe {
        claim_number = claim_register.read_volatile();
    }
    if claim_number == 0 {
        None
    } else {
        Some(claim_number)
    }
}

fn complete(id: u32) {
    let claim_register = PLIC_CLAIM as *mut u32;
    unsafe {
        claim_register.write_volatile(id);
    }
}

fn set_threshold(tsh: u32) {
    let threshold = tsh & 0b111;
    let threshold_regsiter = PLIC_THRESHOLD as *mut u32;
    unsafe {
        threshold_regsiter.write_volatile(threshold);
    }
}

fn enable(id: u32) {
    let int_enable_register = PLIC_INT_ENABLE as *mut u32;
    let desired_id = 1 << id;
    unsafe {
        int_enable_register.write_volatile(int_enable_register.read_volatile() | desired_id);
    }
}

fn set_priority(id: u32, priority: u32) {
    let desired_priority = priority & 0b111;
    let priority_register = PLIC_PRIORITY as *mut u32;
    unsafe {
        priority_register
            .add(id as usize)
            .write_volatile(desired_priority);
    }
}

pub fn init() {
    serial_info("init plic");
    set_threshold(0);
    for i in 1..=8 {
        enable(i);
        set_priority(i, 1);
    }
}

pub fn interrupt_handler() {
    if let Some(interrupt) = next_plic_interrupt() {
        match interrupt {
            1..=8 => {
                virtio::interrupt_handler(interrupt);
            }
            _ => {
                println!("Unhandled external interrupt: {}", interrupt);
            }
        }
        complete(interrupt);
    }
}
