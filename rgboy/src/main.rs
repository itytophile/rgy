mod debug;
mod hardware;

use crate::{debug::Debugger, hardware::Hardware};

use std::fs::File;
use std::io::prelude::*;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
pub struct Opt {
    /// Cpu frequency
    #[structopt(short = "f", long = "freq", default_value = "4200000")]
    freq: u64,
    /// Sampling rate for cpu frequency controller
    #[structopt(short = "s", long = "sample", default_value = "4000")]
    sample: u64,
    /// Delay unit for cpu frequency controller
    #[structopt(short = "u", long = "delayunit", default_value = "10")]
    delay_unit: u64,
    /// Don't adjust cpu frequency
    #[structopt(short = "n", long = "native")]
    native_speed: bool,
    /// Enable debug mode
    #[structopt(short = "d", long = "debug")]
    debug: bool,
    #[structopt(name = "ROM")]
    rom: String,
}

fn load_rom(name: &str) -> Vec<u8> {
    let mut f = File::open(name).expect("Couldn't open file");
    let mut buf = Vec::new();

    f.read_to_end(&mut buf).expect("Couldn't read file");

    buf
}

fn to_cfg(opt: Opt) -> librgboy::Config {
    librgboy::Config::new()
        .freq(opt.freq)
        .sample(opt.sample)
        .delay_unit(opt.delay_unit)
        .native_speed(opt.native_speed)
}

fn main() {
    let opt = Opt::from_args();

    env_logger::init();

    let hw = Hardware::new();
    let rom = load_rom(&opt.rom);

    if opt.debug {
        librgboy::run_debug(to_cfg(opt), rom, hw, Debugger::new());
    } else {
        librgboy::run(to_cfg(opt), rom, hw);
    }
}