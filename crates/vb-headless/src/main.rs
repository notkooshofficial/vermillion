use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use vb_core::bus::Bus;
use vb_core::cart::{Cart, MAX_ROM_LEN, MAX_SRAM_LEN};
use vb_core::cpu::decode::instruction_width;
use vb_core::cpu::exec::{StepOutcome, Stop};
use vb_core::cpu::state::{Cpu, PSW_CY, PSW_OV, PSW_S, PSW_Z};

const DEFAULT_STEPS: u64 = 100;

// roms loop forever by design, so an unbounded run needs a backstop
const SAFETY_CAP: u64 = 10_000_000;
const FLAG_MASK: u32 = PSW_Z | PSW_S | PSW_OV | PSW_CY;

#[derive(Parser)]
#[command(
    name = "vb-headless",
    version,
    about = "run virtual boy roms without a display"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// execute a rom and print one line per instruction
    Trace {
        rom: PathBuf,
        /// stop after this many instructions
        #[arg(long)]
        steps: Option<u64>,
        /// run until the cpu halts or faults
        #[arg(long)]
        until_halt: bool,
        /// stop after this many cpu cycles
        #[arg(long)]
        max_cycles: Option<u64>,
        /// cartridge ram size in bytes
        #[arg(long)]
        sram: Option<usize>,
    },
    /// print the rom header
    Info { rom: PathBuf },
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    let result = match cli.command {
        Command::Trace {
            rom,
            steps,
            until_halt,
            max_cycles,
            sram,
        } => trace(&rom, steps, until_halt, max_cycles, sram),
        Command::Info { rom } => info(&rom),
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("vb-headless: {message}");
            ExitCode::FAILURE
        }
    }
}

// size is checked before reading so an oversized file cannot exhaust memory
fn load_rom(path: &Path) -> Result<Vec<u8>, String> {
    let metadata = fs::metadata(path).map_err(|error| format!("{}: {error}", path.display()))?;

    if metadata.len() > MAX_ROM_LEN as u64 {
        return Err(format!(
            "{}: {} bytes exceeds the {MAX_ROM_LEN} byte maximum",
            path.display(),
            metadata.len()
        ));
    }

    fs::read(path).map_err(|error| format!("{}: {error}", path.display()))
}

fn open(path: &Path, sram: Option<usize>) -> Result<Cart, String> {
    let rom = load_rom(path)?;
    let sram = sram.unwrap_or(0);

    if sram > MAX_SRAM_LEN {
        return Err(format!(
            "sram of {sram} bytes exceeds the {MAX_SRAM_LEN} byte maximum"
        ));
    }

    Cart::with_sram(rom, sram).map_err(|error| format!("{}: {error}", path.display()))
}

fn info(path: &Path) -> Result<(), String> {
    let cart = open(path, None)?;
    let header = cart.header();

    println!("title       {}", header.title_ascii_lossy());
    println!("maker       {}", header.maker_code_ascii_lossy());
    println!("game code   {}", header.game_code_ascii_lossy());
    println!("version     1.{}", header.version);
    println!("rom bytes   {}", cart.rom().len());
    println!("sram bytes  {}", cart.sram().len());

    Ok(())
}

fn trace(
    path: &Path,
    steps: Option<u64>,
    until_halt: bool,
    max_cycles: Option<u64>,
    sram: Option<usize>,
) -> Result<(), String> {
    let mut bus = Bus::new(open(path, sram)?);
    let mut cpu = Cpu::new();

    let step_limit = match (steps, until_halt, max_cycles) {
        (Some(steps), _, _) => steps,
        (None, false, None) => DEFAULT_STEPS,
        (None, _, _) => SAFETY_CAP,
    };

    let mut executed: u64 = 0;

    loop {
        if executed >= step_limit {
            if steps.is_none() && step_limit == SAFETY_CAP {
                eprintln!("-- safety cap of {SAFETY_CAP} instructions hit, rom never halted");
            } else {
                eprintln!("-- stopped after {executed} instructions");
            }
            return Ok(());
        }
        if max_cycles.is_some_and(|limit| cpu.cycles >= limit) {
            eprintln!("-- stopped after {} cycles", cpu.cycles);
            return Ok(());
        }

        let pc = cpu.pc;
        let bytes = raw_bytes(&bus, pc);
        let before_regs = cpu.regs;
        let before_psw = cpu.psw;
        let before_cycles = cpu.cycles;

        let outcome = cpu.step(&mut bus);
        bus.tick(cpu.cycles - before_cycles);

        match outcome {
            Ok(StepOutcome::Executed(instruction)) => {
                let line = format!(
                    "{pc:08x}  {:<11}  {:<24}{}",
                    bytes,
                    vb_debug::disassemble(instruction, pc),
                    changes(&before_regs, before_psw, &cpu)
                );
                println!("{}", line.trim_end());
                executed += 1;
            }
            // idling is guaranteed to end, either a device raises or the halt turns permanent
            Ok(StepOutcome::Halted) => {}
            Ok(StepOutcome::Interrupt { source }) => {
                println!(
                    "{pc:08x}  {:<11}  {:<24}-> {:08x}",
                    "",
                    format!("interrupt {}", source.name()),
                    cpu.pc
                );
                executed += 1;
            }
            Ok(StepOutcome::Exception { code }) => {
                println!(
                    "{pc:08x}  {:<11}  {:<24}-> {:08x}",
                    bytes,
                    format!("exception {code:#06x}"),
                    cpu.pc
                );
                executed += 1;
            }
            Err(stop) => {
                eprintln!("-- {} after {executed} instructions", describe(stop));
                return Ok(());
            }
        }
    }
}

fn raw_bytes(bus: &Bus, pc: u32) -> String {
    let width = instruction_width(bus.read_u16(pc));
    let mut text = String::new();

    for offset in 0..width {
        if offset != 0 {
            text.push(' ');
        }
        let _ = write!(text, "{:02x}", bus.read_u8(pc.wrapping_add(offset)));
    }

    text
}

fn changes(before_regs: &[u32; 32], before_psw: u32, cpu: &Cpu) -> String {
    let mut text = String::new();

    for (index, (before, after)) in before_regs.iter().zip(cpu.regs.iter()).enumerate() {
        if before != after {
            let _ = write!(text, " r{index}={after:08x}");
        }
    }

    let changed = before_psw ^ cpu.psw;

    if changed & FLAG_MASK != 0 {
        let _ = write!(
            text,
            " z{} s{} ov{} cy{}",
            u8::from(cpu.psw & PSW_Z != 0),
            u8::from(cpu.psw & PSW_S != 0),
            u8::from(cpu.psw & PSW_OV != 0),
            u8::from(cpu.psw & PSW_CY != 0)
        );
    }

    if changed & !FLAG_MASK != 0 {
        let _ = write!(text, " psw={:08x}", cpu.psw);
    }

    text
}

fn describe(stop: Stop) -> String {
    match stop {
        Stop::Halted => "halted with nothing able to wake it".to_string(),
        Stop::Fatal { code, pc } => format!("fatal exception {code:#06x} at {pc:08x}"),
        Stop::Unimplemented { op, pc } => {
            format!("unimplemented instruction {} at {pc:08x}", op.mnemonic())
        }
    }
}
