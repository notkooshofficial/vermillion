A virtual boy emulator written in rust, emulating the nec v810 cpu, the vip display processor and the vsu sound unit. Early development, cpu runs but nothing is drawn yet

The virtual boy draws two 384x224 images in four shades of red, one per eye and the core does no i/o and is deterministic, so a rom produces the same frames every run

Needs a stable rust toolchain, plus libasound2-dev and libudev-dev on linux

```
cargo build --release
```


Tracing a rom prints one line per instruction:

```
vb-headless trace rom.vb --steps 4
```

fffffff0 20 bc 00 05 movhi 0x500, r0, r1 r1=05000000
fffffff4 21 a0 00 01 movea 0x100, r1, r1 r1=05000100
fffffff8 45 40 mov 0x5, r2 r2=00000005
fffffffa 42 04 add r2, r2 r2=0000000a

no games are included and the virtual boy has no bios, so a rom file is all you need