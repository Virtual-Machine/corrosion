use crate::alloc;
use crate::block;
use crate::print;
use crate::println;
use crate::uart::{serial_step, serial_test, serial_test_passed};

// mod test.rs
// A collection of tests to run after initialization to ensure things are running as expected.
// Requires --feature "test_suite"

pub fn run() {
    serial_step("Running tests...");
    test_block_device();
    test_traps();
}

fn test_block_device() {
    serial_test("block driver...");
    let buffer = alloc::alloc_bytes(512);
    block::read(buffer, 512, 512 * 2);
    #[cfg(feature = "debug-full")]
    alloc::debug_heap();
    unsafe {
        assert!(buffer.add(0).read() == 0xb0);
        assert!(buffer.add(1).read() == 0x2a);
    }
    alloc::free_bytes(buffer);
    serial_test_passed();
}

fn test_traps() {
    serial_test("traps...");

    let v = core::ptr::null_mut::<u64>();
    unsafe {
        println!("    - Should trigger: Store & Load Access Faults");
        v.write_volatile(0);
        v.read_volatile();
    }

    serial_test_passed();
}
