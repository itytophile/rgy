mod debug;
mod hardware;
mod loader;

use crate::{
    debug::Debugger,
    hardware::Hardware,
    loader::{load_rom, Loader},
};

use log::*;
use rgy::{debug::NullDebugger, Config};
use std::{path::PathBuf, time::Duration};
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
pub struct Opt {
    /// Cpu frequency
    #[structopt(short = "f", long = "freq", default_value = "4194304")]
    freq: u64,
    /// Sampling rate for cpu frequency controller
    #[structopt(short = "s", long = "sample", default_value = "4200")]
    sample: u64,
    /// Delay unit for cpu frequency controller
    #[structopt(short = "u", long = "delayunit", default_value = "1")]
    delay_unit: u64,
    /// Don't adjust cpu frequency
    #[structopt(short = "n", long = "native")]
    native_speed: bool,
    /// Enable debug mode
    #[structopt(short = "d", long = "debug")]
    debug: bool,
    /// RAM file name
    #[structopt(short = "r", long = "ram")]
    ram: Option<String>,
    /// ROM file name or directory
    #[structopt(name = "ROM")]
    rom: PathBuf,
}

fn to_cfg(opt: Opt) -> rgy::Config {
    rgy::Config::new()
        .freq(opt.freq)
        .sample(opt.sample)
        .delay_unit(opt.delay_unit)
        .native_speed(opt.native_speed)
}

fn set_affinity() {
    let set = || {
        let core_ids = core_affinity::get_core_ids()?;
        core_affinity::set_for_current(*core_ids.first()?);
        Some(())
    };

    if set().is_none() {
        warn!("Couldn't set CPU affinity")
    }
}

fn main() {
    let opt = Opt::from_args();

    env_logger::init();

    let hw = Hardware::new(opt.ram.clone());
    let hw1 = hw.clone();

    std::thread::spawn(move || {
        let (rom, hw) = if opt.rom.is_dir() {
            let mut ldr = Loader::new(&opt.rom);

            utils::select(&mut ldr, hw1)
        } else {
            (load_rom(&opt.rom), hw1)
        };

        set_affinity();

        if opt.debug {
            run(Debugger::new(), hw, &rom, to_cfg(opt));
        } else {
            run(NullDebugger, hw, &rom, to_cfg(opt));
        }
    });

    hw.run();
}

fn run<D: rgy::debug::Debugger + 'static, H: rgy::Hardware + 'static>(
    debugger: D,
    hw: H,
    rom: &[u8],
    cfg: Config,
) {
    let state0 = rgy::system::get_stack_state0(hw, debugger);
    let state1 = rgy::system::get_stack_state1(&state0, rom);
    let devices = rgy::system::Devices::new(&state1.raw_devices);
    let handlers = rgy::system::Handlers::new(devices.clone());
    let mut sys = rgy::System::new(
        cfg,
        state1.hw_handle,
        &state0.dbg_cell,
        &state1.dbg_handler,
        devices.clone(),
        &handlers,
    );
    while let Some(poll_state) = sys.poll() {
        println!("{}", poll_state.delay);
        spin_sleep::sleep(Duration::from_nanos(poll_state.delay));
    }
}
