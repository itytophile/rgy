use std::time::Duration;

use rgy::{sound::MixerStream, Config, Key, VRAM_HEIGHT, VRAM_WIDTH};

struct Hardware;

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

    let mut display = vec![vec![0u32; VRAM_HEIGHT]; VRAM_WIDTH];

    // Create the hardware instance.
    let hw = Hardware;

    // The content of a ROM file, which can be downloaded from the Internet.
    let rom = vec![0u8; 1024];

    // Run the emulator.
    let state0 = rgy::system::get_stack_state0(hw);
    let state1 = rgy::system::get_stack_state1(&state0, &rom);
    let devices = rgy::system::Devices::new(&state1.raw_devices);
    let handlers = rgy::system::Handlers::new(devices.clone());
    let mut sys = rgy::System::new(cfg, state1.hw_handle, devices.clone(), &handlers);
    let mut mixer_stream = MixerStream::default();

    while let Some(poll_state) = sys.poll(&mut mixer_stream) {
        if let Some((line, buffer)) = poll_state.line_to_draw {
            let y = line as usize;

            for (x, col) in buffer.iter().enumerate() {
                display[x][y] = *col;
            }
        }
        spin_sleep::sleep(Duration::from_nanos(poll_state.delay));
    }
}
