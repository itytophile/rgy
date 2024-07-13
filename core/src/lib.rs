//!
//! `rgy` is no-std cross-platform Rust GameBoy emulator library.
//!
//! The users of this library only needs to implement [`Hardware`][] trait, which abstracts OS-specific function.
//! Once it's implemented, the emulator works.
//!
//! The following code is the example which just implements `Hardware`. The implementation does nothing.
//! You can replace the body of each function with the actual meaningful logic.
//!
//! ```rust,no_run
//! use rgy::{Config, Key, Stream, VRAM_HEIGHT, VRAM_WIDTH};
//!
//! struct Hardware {
//!     dummy_display: Vec<Vec<u32>>,
//! }
//!
//! impl Hardware {
//!     fn new() -> Self {
//!         // Create a frame buffer with the size VRAM_WIDTH * VRAM_HEIGHT.
//!         let dummy_display = vec![vec![0u32; VRAM_HEIGHT]; VRAM_WIDTH];
//!
//!         Self { dummy_display }
//!     }
//! }
//!
//! impl rgy::Hardware for Hardware {
//!     // Called when a horizontal line in the display is updated by the emulator.
//!     fn vram_update(&mut self, line: usize, buffer: &[u32]) {
//!         // `line` corresponds to the y coordinate.
//!         let y = line;
//!
//!         for (x, col) in buffer.iter().enumerate() {
//!             // TODO: Update the pixels in the actual display here.
//!             self.dummy_display[x][y] = *col;
//!         }
//!     }
//!
//!     // Called when the emulator checks if a key is pressed or not.
//!     fn joypad_pressed(&mut self, key: Key) -> bool {
//!         println!("Is {:?} pressed?", key);
//!
//!         // TODO: Read a keyboard device and check if the `key` is pressed or not.
//!
//!         false
//!     }
//!
//!     // Provides clock for the emulator.
//!     fn clock(&mut self) -> u64 {
//!         // TODO: Return the epoch in microseconds.
//!         let epoch = std::time::SystemTime::now()
//!             .duration_since(std::time::UNIX_EPOCH)
//!             .expect("Couldn't get epoch");
//!         epoch.as_micros() as u64
//!     }
//!
//!     // Called when the emulator sends a byte to the serial port.
//!     fn send_byte(&mut self, _b: u8) {
//!         // TODO: Send a byte to a serial port.
//!     }
//!
//!     // Called when the emulator peeks a byte from the serial port.
//!     fn recv_byte(&mut self) -> Option<u8> {
//!         // TODO: Check the status of the serial port and read a byte if any.
//!         None
//!     }
//!
//!     // Called every time the emulator executes an instruction.
//!     fn sched(&mut self) -> bool {
//!         // TODO: Do some periodic jobs if any. Return `true` to continue, `false` to stop the emulator.
//!         println!("It's running!");
//!         true
//!     }
//!
//!     // Called when the emulator loads the save data from the battery-backed RAM.
//!     fn save_ram(&mut self, _ram: &[u8]) {
//!         // TODO: Store save data.
//!     }
//! }
//!
//! // Create the default config.
//! let cfg = Config::new();
//!
//! // Create the hardware instance.
//! let hw = Hardware::new();
//!
//! // TODO: The content of a ROM file, which can be downloaded from the Internet.
//! let rom = vec![0u8; 1024];
//!
//! let mut cartridge_ram = [0; 0x8000];
//!
//! let mut sys = rgy::System::<_, rgy::mmu::DmgMode>::new(cfg, &rom, hw, &mut cartridge_ram);
//!
//! let mut mixer_stream = rgy::apu::mixer::MixerStream::new();
//!
//! // Run the emulator.
//! while sys.poll(&mut mixer_stream) {}
//!
//! ```

#![no_std]
// #![warn(missing_docs)]

mod alu;
pub mod apu;
mod cgb;
mod dma;
mod gpu;
mod hram;
mod ic;
mod joypad;
mod mbc;
mod serial;
mod system;
mod timer;
mod wram;

/// CPU state.
pub mod cpu;

/// Adaptor to register devices to MMU.
// pub mod device;

/// Decoder which evaluates each CPU instructions.
pub mod inst;

/// Handles memory and I/O port access from the CPU.
pub mod mmu;

/// Hardware interface, which abstracts OS-specific functions.
mod hardware;

pub use crate::hardware::{Hardware, Key, Stream, VRAM_HEIGHT, VRAM_WIDTH};
pub use crate::system::{Config, System};
