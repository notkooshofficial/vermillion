A virtual boy emulator written in rust, emulating the nec v810 cpu, the vip display processor and the vsu sound unit. Early development, nothing is emulate-able yet.

The virtual boy draws two 384x224 images in four shades of red, one per eye and the core does no i/o and is deterministic, so a rom produces the same frames every run.

Needs a stable rust toolchain, plus libasound2-dev and libudev-dev on linux

```
cargo build --release
```

no games are included and the virtual boy has no bios, so a rom file is all you need