// CorrOSion
// A toy OS

#![no_main]
#![no_std]
#![allow(internal_features)]
#![feature(panic_info_message, alloc_error_handler, lang_items)]

// Project Rust Modules
pub mod alloc;
pub mod assembly;
pub mod block;
pub mod config;
pub mod plic;
pub mod test;
pub mod trap;
pub mod uart;
pub mod virtio;

use crate::uart::serial_step;
use core::arch::asm;

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
        unsafe {
            asm!("wfi");
        }
    }
}

#[no_mangle]
// Interrupts are disabled here...
extern "C" fn kernel_init() {
    uart::init(); // Kick off UART for debugging
    alloc::init(); // Kernel Memory Allocator
    plic::init(); // Platform level interrupt controller
    virtio::init(); // Virtio driver
}

#[no_mangle]
// Interrupts are enabled here...
extern "C" fn kernel_main() {
    #[cfg(feature = "test-suite")]
    test::run();

    serial_step("Booted successfully!\n")
}
