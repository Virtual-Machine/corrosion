use core::arch::{asm, global_asm};

// mod assembly.rs
// This pulls in src/asm/_.S files into cargo build as module level asm
// And provides wrappers for common riscv asm calls

// Incorporate bootloader into rust as a module so cargo can compile it
global_asm!(include_str!("asm/boot.S"));
// Incorporate trap vector
global_asm!(include_str!("asm/trap.S"));
// Incorporate linker symbols
global_asm!(include_str!("asm/layout.S"));

// Wrapper to perform no operation
// Used currently as a crude sleep until multi threaded
pub fn no_operation() {
    unsafe {
        asm!("nop");
    }
}

// Wrapper to wait for an interrupt
// Used to sleep secondary harts in halt loop
pub fn wait_for_interrupt() {
    unsafe {
        asm!("wfi");
    }
}

// Wrapper to trigger an illegal load
// Used to test traps
pub fn trigger_illegal_load() {
    unsafe {
        asm!("li a0, 1", "li a1, 1", "lw a1, 1(a0)");
    }
}

// Wrapper to trigger an illegal store
// Used to test traps
pub fn trigger_illegal_store() {
    unsafe {
        asm!("li a0, 1", "li a1, 1", "sw a1, 1(a0)");
    }
}

// Used to trigger a shutdown in the qemu virt platform
pub fn trigger_shutdown() {
    unsafe {
        asm!("li a0, 0x100000", "li a1, 0x5555", "sw a1, 0(a0)");
    }
}
