use rgy::{apu::mixer::MixerStream, mmu::DmgMode, Config};

struct Hardware;

impl rgy::Hardware for Hardware {
    fn clock(&mut self) -> u64 {
        // Return the epoch in microseconds.
        let epoch = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("Couldn't get epoch");
        epoch.as_micros() as u64
    }

    fn save_ram(&mut self, _ram: &[u8]) {
        // Store save data.
    }
}

fn main() {
    // Create the default config.
    let cfg = Config::new();

    // The content of a ROM file, which can be downloaded from the Internet.
    let rom = vec![0u8; 1024];

    let mut cartridge_ram = [0; 100];
    let mut sys = rgy::System::<_, DmgMode>::new(cfg, &rom, Hardware, &mut cartridge_ram);

    let mut mixer_stream = MixerStream::new();

    loop {
        sys.poll(&mut mixer_stream, Default::default(), &mut None);
    }
}
