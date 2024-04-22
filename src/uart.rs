use crate::config::{BANNER, DEBUG, INFO, MAIN, STEP, TEST, TEST_PASSED, VERSION, PLATFORM};
use crate::print;
use crate::println;
use core::fmt::{Error, Write};

// mod uart.rs
// This is a particularly limited driver for printing to the riscv QEMU virt serial device
// It will strictly be used for debugging and therefore is particularly limited

static mut UART: Uart = Uart {
    base_address: 0x1000_0000,
};

pub struct Uart {
    base_address: usize,
}

impl Write for Uart {
    fn write_str(&mut self, out: &str) -> Result<(), Error> {
        for c in out.bytes() {
            self.put(c);
        }
        Ok(())
    }
}

const BASE: usize = 0;
const IER: usize = 1; // interrupt enable register
const FCR: usize = 2; // FIFO control register
const LCR: usize = 3; // line control register
const BI0: u8 = 1; // Bit index 0 (1 << 0)
const BI0A1: u8 = 3; // Bit indexes 0+1 (1 << 0) | (1 << 1)

impl Uart {
    pub fn init(&mut self) {
        let ptr = self.base_address as *mut u8;
        unsafe {
            ptr.add(LCR).write_volatile(BI0A1);
            ptr.add(FCR).write_volatile(BI0);
            ptr.add(IER).write_volatile(BI0);
        }
        Uart::print_banner();
        serial_main(VERSION);
        serial_main(PLATFORM);
        serial_step("Booting...");
    }

    fn print_banner() {
        println!("{}", BANNER);
    }

    pub fn put(&mut self, c: u8) {
        let ptr = self.base_address as *mut u8;
        unsafe {
            ptr.add(BASE).write_volatile(c);
        }
    }
}

pub fn init() {
    unsafe { UART.init() }
}

pub fn get_uart() -> &'static mut Uart {
    unsafe { &mut UART }
}

pub fn serial_info(txt: &str) {
    println!("  {} {}", INFO, txt);
}

pub fn serial_main(txt: &str) {
    println!("{} {}", MAIN, txt);
}

pub fn serial_step(txt: &str) {
    println!("\n{} {}", STEP, txt);
}

pub fn serial_test(txt: &str) {
    println!("  {} {}", TEST, txt);
}

pub fn serial_debug(txt: &str) {
    println!("\n  {} {}", DEBUG, txt);
}

pub fn serial_test_passed() {
    println!("{}", TEST_PASSED);
}
