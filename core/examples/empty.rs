use rgy::{apu::mixer::MixerStream, mmu::DmgMode, Config, Key, VRAM_HEIGHT, VRAM_WIDTH};

struct Hardware {
    display: Vec<Vec<u32>>,
}

impl Hardware {
    fn new() -> Self {
        // Create a frame buffer with the size VRAM_WIDTH * VRAM_HEIGHT.
        let display = vec![vec![0u32; VRAM_HEIGHT]; VRAM_WIDTH];

        Self { display }
    }
}

impl rgy::Hardware for Hardware {
    fn joypad_pressed(&mut self, key: Key) -> bool {
        // Read a keyboard device and check if the `key` is pressed or not.
        println!("Check if {:?} is pressed", key);
        false
    }

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

    // Create the hardware instance.
    let hw = Hardware::new();

    // The content of a ROM file, which can be downloaded from the Internet.
    let rom = vec![0u8; 1024];

    let mut cartridge_ram = [0; 100];
    let mut sys = rgy::System::<_, DmgMode>::new(cfg, &rom, hw, &mut cartridge_ram);

    let mut mixer_stream = MixerStream::new();

    while sys.poll(&mut mixer_stream) {}
}
