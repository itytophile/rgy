mod hardware;

use crate::hardware::Hardware;

use rgy::{sound::MixerStream, VRAM_HEIGHT, VRAM_WIDTH};
use std::{
    fs::File,
    io::Read,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
pub struct Opt {
    /// RAM file name
    #[structopt(short = "r", long = "ram")]
    ram: Option<String>,
    /// ROM file name or directory
    #[structopt(name = "ROM")]
    rom: PathBuf,
}

pub fn load_rom<P: AsRef<Path>>(path: P) -> Vec<u8> {
    let mut f = File::open(path).expect("Couldn't open file");
    let mut buf = Vec::new();

    f.read_to_end(&mut buf).expect("Couldn't read file");

    buf
}

fn main() {
    let opt = Opt::from_args();

    env_logger::init();

    let mixer_stream = Arc::new(Mutex::new(MixerStream::default()));
    let hw = Hardware::new(opt.ram.clone(), mixer_stream.clone());
    let vram = hw.vram.clone();

    let hw1 = hw.clone();

    std::thread::spawn(move || {
        let (rom, hw) = (load_rom(&opt.rom), hw1);

        run(hw, &rom, vram, mixer_stream);
    });

    hw.run();
}

fn run<H: rgy::Hardware + 'static>(
    hw: H,
    rom: &[u8],
    vram: Arc<Mutex<Vec<u32>>>,
    mixer_stream: Arc<Mutex<MixerStream>>,
) {
    let state0 = rgy::system::get_stack_state0(hw);
    let state1 = rgy::system::get_stack_state1(&state0);
    let mut sys = rgy::System::new(state1.hw_handle, rom);

    let mut lock = None;
    let mut irq = Default::default();

    while let Some(poll_state) = sys.poll(&mut mixer_stream.lock().unwrap(), &mut irq) {
        if let Some((line, buf)) = poll_state.line_to_draw {
            let mut vram = lock.unwrap_or_else(|| vram.lock().unwrap());
            let base = line as usize * VRAM_WIDTH;
            vram[base..base + buf.len()].copy_from_slice(&buf);
            lock = if line == VRAM_HEIGHT as u8 - 1 {
                drop(vram);
                std::thread::sleep(Duration::from_millis(16));
                None
            } else {
                Some(vram)
            };
        }
    }
}
