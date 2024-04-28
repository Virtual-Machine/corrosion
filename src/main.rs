// CorrOSion
// A toy OS

#![no_main]
#![no_std]
#![allow(internal_features)]
#![feature(panic_info_message, alloc_error_handler, lang_items)]

// Project Rust Modules
mod alloc;
mod assembly;
mod block;
mod buffer;
mod config;
mod debug;
mod memory;
mod minixfs3;
mod plic;
#[allow(unused_imports)]
mod test;
mod trap;
mod uart;
mod virtio;

use crate::uart::serial_step;

extern crate alloc as rust_alloc;

#[lang = "eh_personality"]
extern "C" fn eh_personality() {}

#[macro_export]
macro_rules! print
{
    ($($args:tt)+) => ({
            use core::fmt::Write;
                let _ = write!($crate::uart::get_uart(), $($args)+);
            });
}
#[macro_export]
macro_rules! println
{
    () => ({
           print!("\r\n")
           });
    ($fmt:expr) => ({
            print!(concat!($fmt, "\r\n"))
            });
    ($fmt:expr, $($args:tt)+) => ({
            print!(concat!($fmt, "\r\n"), $($args)+)
            });
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    print!("Aborting: ");
    if let Some(p) = info.location() {
        println!(
            "line {}, file {}: {}",
            p.line(),
            p.file(),
            info.message().unwrap()
        );
    } else {
        println!("no information available.");
    }
    abort();
}
#[no_mangle]
extern "C" fn abort() -> ! {
    loop {
        assembly::wait_for_interrupt();
    }
}

#[no_mangle]
// Interrupts are disabled here...
extern "C" fn kernel_init() {
    uart::init(); // Kick off UART for debugging
    alloc::init(); // Kernel Memory Allocator
    plic::init(); // Platform level interrupt controller
    virtio::init(); // Virtio driver
    minixfs3::init(); // Initialize fs cache
    #[cfg(feature = "debug-full")]
    debug::fs_cache();
}

#[no_mangle]
// Interrupts are enabled here...
extern "C" fn kernel_main() {
    #[cfg(feature = "test-suite")]
    test::run();

    serial_step("Booted successfully!\n")
}
