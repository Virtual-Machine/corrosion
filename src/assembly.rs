use core::arch::global_asm;

// mod assembly.rs
// This pulls in src/asm/_.S files into cargo build as module level asm

// Incorporate bootloader into rust as a module so cargo can compile it
global_asm!(include_str!("asm/boot.S"));
// Incorporate trap vector
global_asm!(include_str!("asm/trap.S"));
// Incorporate linker symbols
global_asm!(include_str!("asm/layout.S"));
