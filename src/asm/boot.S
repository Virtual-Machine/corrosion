# Minimal bootloader for corrOSion
# Depends on linker script to be loaded into the virt start address
.option norvc

.section .text.init

.global _start
_start:

.option push
.option norelax
	la		gp, _global_pointer
.option pop
	csrw	satp, zero
	csrr	t0, mhartid
	bnez	t0, _kernel_halt
_zero_bss_init:
	la 		a0, _bss_start
	la		a1, _bss_end
	bgeu	a0, a1, _machine_setup
_zero_bss_main:
	sd		zero, (a0)
	addi	a0, a0, 8
	bltu	a0, a1, _zero_bss_main
_machine_setup:
	la		sp, _stack_top
	li		t0, (0b11 << 11) | (1 << 13)
	csrw	mstatus, t0
	csrw	mie, zero
	la		t1, kernel_init
	csrw	mepc, t1
	la		ra, _machine_main
	mret
_machine_main:
	li		t0, (0b11 << 11) | (1 << 7) | (1 << 5) | (1 << 13)
	csrw	mstatus, t0
	la		t1, _machine_trap_asm
	csrw	mtvec, t1
	la		t2, kernel_main
	csrw	mepc, t2
	li		t3, 0xfff
	csrw	mie, t3
	la      ra, _kernel_halt
	mret
_kernel_halt:
	wfi
	j		_kernel_halt

