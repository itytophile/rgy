// mod debug;
mod hardware;
mod loader;

use crate::{
    // debug::Debugger,
    hardware::Hardware,
    loader::load_rom,
};

use log::*;
use rgy::{apu::mixer::MixerStream, hardware::JoypadInput, mmu::DmgMode, VRAM_HEIGHT, VRAM_WIDTH};
use std::{
    fs::File,
    io::Read,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
pub struct Opt {
    /// Emulate Gameboy Color
    #[structopt(short = "c", long = "color")]
    color: bool,
    /// Cpu frequency
    #[structopt(short = "f", long = "freq", default_value = "4200000")]
    freq: u64,
    /// Sampling rate for cpu frequency controller
    #[structopt(short = "s", long = "sample", default_value = "4200")]
    sample: u64,
    /// Delay unit for cpu frequency controller
    #[structopt(short = "u", long = "delayunit", default_value = "50")]
    delay_unit: u64,
    /// Don't adjust cpu frequency
    #[structopt(short = "n", long = "native")]
    native_speed: bool,
    /// RAM file name
    #[structopt(short = "r", long = "ram")]
    ram: Option<String>,
    /// ROM file name or directory
    #[structopt(name = "ROM")]
    rom: PathBuf,
}

fn to_cfg(opt: Opt) -> rgy::Config {
    rgy::Config::new()
        .color(opt.color)
        .freq(opt.freq)
        .sample(opt.sample)
        .delay_unit(opt.delay_unit)
}

fn main() {
    let opt = Opt::from_args();

    use std::io::Write;

    let mut builder = env_logger::Builder::from_default_env();

    builder
        .format(|buf, record| {
            let ts = buf.timestamp_millis();

            writeln!(buf, "{}: {}: {}", ts, record.level(), record.args())
        })
        .init();
    // env_logger::init();

    let mixer_stream = Arc::new(Mutex::new(MixerStream::new()));

    let vram = Arc::new(Mutex::new(vec![0; VRAM_WIDTH * VRAM_HEIGHT]));
    let joypad_input = Arc::new(Mutex::new(JoypadInput::default()));
    let hw = Hardware::new(
        opt.ram.clone(),
        opt.color,
        mixer_stream.clone(),
        vram.clone(),
        joypad_input.clone(),
    );
    let hw1 = hw.clone();

    let handle = std::thread::spawn(move || {
        let rom = load_rom(&opt.rom);

        let mut ram = vec![0; 0x20000];

        if let Some(path) = &opt.ram {
            match File::open(path) {
                Ok(mut fs) => {
                    fs.read_exact(&mut ram).expect("Couldn't read file");
                }
                Err(e) => {
                    warn!("Couldn't open RAM file `{}`: {}", path, e);
                }
            }
        }

        let native_speed = opt.native_speed;

        let mut sys = rgy::System::<_, DmgMode>::new(to_cfg(opt), &rom, hw1, &mut ram);

        let mut max_cycles = 0;

        while let Some(poll_data) = sys.poll(
            &mut mixer_stream.lock().unwrap(),
            *joypad_input.lock().unwrap(),
        ) {
            max_cycles = max_cycles.max(poll_data.cpu_time);
            if let Some((ly, buf)) = poll_data.line_to_draw {
                let mut vram = vram.lock().unwrap();
                let base = usize::from(ly) * VRAM_WIDTH;
                for (a, b) in vram[base..].iter_mut().zip(buf) {
                    *a = u32::from(*b);
                }
                drop(vram);
                if usize::from(ly) == VRAM_HEIGHT - 1 && !native_speed {
                    std::thread::sleep(Duration::from_millis(16))
                }
            }
        }
        dbg!(max_cycles);
    });

    hw.run();
    handle.join().unwrap();
}
