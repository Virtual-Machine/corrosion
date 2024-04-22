use crate::config::{RESET_COLOUR, TRAP_COLOUR};
use crate::plic;
use crate::print;
use crate::println;

// mod trap.rs
// Rust handler switch for CPU traps

// machine_trap_rust is called from _machine_trap_asm
// see src/asm/trap.S

// Async
const MACHINE_SOFTWARE_INTERRUPT: usize = 3;
const MACHINE_TIMER_INTERRUPT: usize = 7;
const MACHINE_EXTERNAL_INTERRUPT: usize = 11;
// Sync
const ILLEGAL_INSTRUCTION: usize = 2;
const LOAD_ACCESS_FAULT: usize = 5;
const STORE_ACCESS_FAULT: usize = 7;
const USER_ECALL: usize = 8;
const SUPERVISOR_ECALL: usize = 9;
const MACHINE_ECALL: usize = 11;

#[no_mangle]
extern "C" fn machine_trap_rust(epc: usize, tval: usize, cause: usize, hart: usize) -> usize {
    let is_async = cause >> 63 & 1 == 1;
    let cause_index = cause & 0xfff;
    let mut pc = epc;
    if is_async {
        match cause_index {
            MACHINE_SOFTWARE_INTERRUPT => {
                println!(
                    "{}Machine software interrupt\n\tCPU#{}{}",
                    TRAP_COLOUR, hart, RESET_COLOUR
                );
            }
            MACHINE_TIMER_INTERRUPT => unsafe {
                let mtimecmp = 0x0200_4000 as *mut u64;
                let mtime = 0x0200_bff8 as *const u64;
                mtimecmp.write_volatile(mtime.read_volatile() + 10_000_000);
                // println!(".");
            },
            MACHINE_EXTERNAL_INTERRUPT => {
                // println!("Machine external interrupt from PLIC\n\tCPU#{}", hart);
                plic::interrupt_handler();
            }
            _ => {
                panic!("Unhandled async trap\n\tCPU#{} -> {}\n", hart, cause_index);
            }
        }
    } else {
        match cause_index {
            ILLEGAL_INSTRUCTION => {
                panic!(
                    "Illegal instruction\n\tCPU#{} -> 0x{:08x}: 0x{:08x}\n",
                    hart, epc, tval
                );
            }
            LOAD_ACCESS_FAULT => {
                println!(
                    "{}Load access fault\n\tCPU#{} -> 0x{:08x}{}",
                    TRAP_COLOUR, hart, epc, RESET_COLOUR
                );
            }
            STORE_ACCESS_FAULT => {
                println!(
                    "{}Store / AMO access fault\n\tCPU#{} -> 0x{:08x}{}",
                    TRAP_COLOUR, hart, epc, RESET_COLOUR
                );
            }
            USER_ECALL => {
                println!(
                    "{}E-call from User mode!\n\tCPU#{} -> 0x{:08x}{}",
                    TRAP_COLOUR, hart, epc, RESET_COLOUR
                );
            }
            SUPERVISOR_ECALL => {
                println!(
                    "{}E-call from Supervisor mode!\n\tCPU#{} -> 0x{:08x}{}",
                    TRAP_COLOUR, hart, epc, RESET_COLOUR
                );
            }
            MACHINE_ECALL => {
                panic!(
                    "{}E-call from Machine mode!\n\tCPU#{} -> 0x{:08x}{}\n",
                    TRAP_COLOUR, hart, epc, RESET_COLOUR
                );
            }
            _ => {
                panic!("Unhandled sync trap\n\tCPU#{} -> {}\n", hart, cause_index);
            }
        }
        pc += 4;
    };
    pc
}
