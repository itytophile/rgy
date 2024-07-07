use rgy::{sound::MixerStream, Key, VRAM_HEIGHT, VRAM_WIDTH};

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

    fn save_ram(&mut self, _ram: &[u8]) {
        // Store save data.
    }
}

fn main() {
    let mut display = vec![vec![0u32; VRAM_HEIGHT]; VRAM_WIDTH];

    // Create the hardware instance.
    let hw = Hardware;

    // The content of a ROM file, which can be downloaded from the Internet.
    let rom = vec![0u8; 1024];

    // Run the emulator.
    let state0 = rgy::system::get_stack_state0(hw);
    let state1 = rgy::system::get_stack_state1(&state0);
    let mut sys = rgy::System::new(state1.hw_handle, &rom);
    let mut mixer_stream = MixerStream::default();
    let mut irq = Default::default();

    loop {
        let poll_state = sys.poll(&mut mixer_stream, &mut irq);
        if let Some((line, buffer)) = poll_state.line_to_draw {
            let y = line as usize;

            for (x, col) in buffer.iter().enumerate() {
                display[x][y] = *col;
            }
        }
    }
}
