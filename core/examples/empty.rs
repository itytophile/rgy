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

    fn send_byte(&mut self, _b: u8) {
        // Send a byte to a serial port.
    }

    fn recv_byte(&mut self) -> Option<u8> {
        // Try to read a byte from a serial port.
        None
    }

    fn sched(&mut self) -> bool {
        // `true` to continue, `false` to stop the emulator.
        println!("It's running!");
        true
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

    while sys.poll(&mut mixer_stream, Default::default()).is_some() {}
}
