# Corrosion

[https://img.shields.io/badge/version-0.2.0-blue]

## CorrOSion is an OS

-   Language: Rust
-   CPU Arch: Risc-V
-   Bits: 64
-   Machine: Qemu Virt

Please note that this is a toy and not planned for serious use. Regardless, this should be a fun learning/experimentation resource for those interested.

## Attribution

For transparency, corrOSion is heavily inspired by Stephen Marz' osblog. Whenever code between the projects happens to be largely indistinguishable, all credit and thanks belongs to Stephen Marz. However corrOSion is NOT a fork of osblog. CorrOSion can be thought of more like a ground up re-write with several different project goals, architectural decisions, and style guidelines in place.

## Roadmap

-   0.1.0 [x] UART, PLIC, Traps, VirtIO Block Driver, Kernel Page and Byte Allocator
-   0.2.0 [ ] FileSystem
-   0.3.0 [ ] Graphics, Keyboard Input
-   0.?.0 [ ] Networking, Mouse Input, APLIC, SMP

## Getting Started

Install prerequisites:

1. Nightly Rust.
2. Riscv64 QEMU.

Run the OS with features as desired:

```bash
make run      # To run the OS
# --OR--
make run-test # To run the OS with the test suite enabled
# --OR--
make run-debug # To run the OS with the test suite and debugging enabled
```

## Going Further

1. Make changes to the source.
2. Run the OS to see your changes reflected.
3. If you have something interesting. Consider submitting a PR.
4. PRs for proposals, discussions, bug fixes, and new features are welcome.
