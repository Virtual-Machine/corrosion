.option norvc

# Macros for saving/loading gp regs to/from memory
.altmacro
.set REG_SIZE, 8

.macro save_gp i, basereg=sp
	sd	x\i, ((\i)*REG_SIZE)(\basereg)
.endm
.macro load_gp i, basereg=sp
	ld	x\i, ((\i)*REG_SIZE)(\basereg)
.endm

# _machine_trap_asm is triggered by the CPU automatically
# when there is an interrupt or exception that is active...
# it calls machine_trap_rust to perform the core logic
# see src/trap.rs

.section .text
.global _machine_trap_asm
.align 4
_machine_trap_asm:

    addi sp, sp, -256

	.set 	i, 1
	.rept	31
		save_gp	%i
		.set	i, i+1
	.endr

	csrr	a0, mepc
	csrr	a1, mtval
	csrr	a2, mcause
	csrr	a3, mhartid
    call	machine_trap_rust
    csrw	mepc, a0

    .set	i, 1
	.rept	31
		load_gp %i
		.set	i, i+1
	.endr
    
    addi sp, sp, 256
    mret