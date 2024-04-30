use crate::alloc;
use crate::assembly;
use crate::block;
use crate::debug;
use crate::minixfs3::MinixFileSystem;
use crate::uart::{serial_step, serial_test, serial_test_passed};
use crate::{print, println};

// mod test.rs
// A collection of tests to run after initialization to ensure things are running as expected.
// Requires --feature "test_suite"
#[allow(dead_code)]
pub fn run() {
    serial_step("Running tests...");
    test_traps();
    test_block_device_stress();
    test_block_device_read();
    #[cfg(feature = "test-block-write")]
    test_block_device_write();
    test_minixfs3_stress();
    test_minixfs3_read();
    test_minixfs3_read_file();
}

#[allow(dead_code)]
fn test_traps() {
    serial_test("traps...");

    println!("Should trigger an illegal load...");
    assembly::trigger_illegal_load();
    println!("...[ok]");

    println!("Should trigger an illegal store...");
    assembly::trigger_illegal_store();
    println!("...[ok]");

    serial_test_passed();
}

#[allow(dead_code)]
fn test_block_device_stress() {
    serial_test("block driver stress...");
    let buffer = alloc::alloc_bytes(512);
    for _ in 0..1000 {
        block::read(buffer, 512, 512 * 2);
        unsafe {
            assert!(buffer.add(0).read() == 0xb0);
            assert!(buffer.add(1).read() == 0x2a);
        }
    }
    alloc::free_bytes(buffer);
    serial_test_passed();
}

#[allow(dead_code)]
fn test_block_device_read() {
    serial_test("block driver read...");
    let buffer = alloc::alloc_bytes(512);
    block::read(buffer, 512, 512 * 2);
    #[cfg(feature = "debug-full")]
    debug::heap();
    unsafe {
        assert!(buffer.add(0).read() == 0xb0);
        assert!(buffer.add(1).read() == 0x2a);
    }
    alloc::free_bytes(buffer);
    serial_test_passed();
}

#[allow(dead_code)]
fn test_block_device_write() {
    serial_test("block driver write...");
    let buffer = alloc::alloc_bytes_zeroed(512);
    block::write(buffer, 512, 0);
    alloc::free_bytes(buffer);
    serial_test_passed();
}

#[allow(dead_code)]
fn test_minixfs3_stress() {
    serial_test("test minixfs stress...");

    for _ in 0..100 {
        MinixFileSystem::get_inode(1);
    }

    serial_test_passed();
}

#[allow(dead_code)]
fn test_minixfs3_read() {
    const FILE_SIZE: u32 = 3;
    serial_test("minix3 fs driver read...");
    let buffer = alloc::alloc_bytes(100);
    let inode = MinixFileSystem::get_inode(2);
    if let Some(node) = inode {
        let bytes_read = MinixFileSystem::read(&node, buffer, 100, 0);
        if bytes_read != FILE_SIZE {
            for i in 0..100 {
                print!("{}", unsafe { buffer.add(i).read() } as char);
            }
            println!(
                "Read {} bytes, but I thought the file was 3 bytes.",
                bytes_read
            );
        } else {
            unsafe {
                assert!(buffer.add(0).read() == b'h');
                assert!(buffer.add(1).read() == b'i');
            }
            serial_test_passed();
        }
    } else {
        println!("Unable to find node 2");
    }
    alloc::free_bytes(buffer);
}

#[allow(dead_code)]
fn test_minixfs3_read_file() {
    const FILE_SIZE: u32 = 3;
    serial_test("minix3 fs driver read file...");
    let buffer = alloc::alloc_bytes(100);

    let bytes_read = MinixFileSystem::read_file("/hello.txt", buffer, 100, 0);
    if bytes_read != FILE_SIZE {
        for i in 0..100 {
            print!("{}", unsafe { buffer.add(i).read() } as char);
        }
        println!(
            "Read {} bytes, but I thought the file was 3 bytes.",
            bytes_read
        );
    } else {
        unsafe {
            assert!(buffer.add(0).read() == b'h');
            assert!(buffer.add(1).read() == b'i');
        }
        serial_test_passed();
    }
    alloc::free_bytes(buffer);
}
