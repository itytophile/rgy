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
//!         let dummy_display = vec![vec![0u32; usize::from(VRAM_HEIGHT)]; usize::from(VRAM_WIDTH)];
//!
//!         Self { dummy_display }
//!     }
//! }
//!
//! impl rgy::Clock for Hardware {
//!     // Provides clock for the emulator.
//!     fn clock(&self) -> u64 {
//!         let epoch = std::time::SystemTime::now()
//!             .duration_since(std::time::UNIX_EPOCH)
//!             .expect("Couldn't get epoch");
//!         epoch.as_micros() as u64
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
//! loop {
//!     sys.poll(&mut mixer_stream, Default::default(), &mut None);
//! }
//!
//! ```

#![no_std]
// #![warn(missing_docs)]

mod alu;
pub mod apu;
mod cgb;
mod dma;
pub mod gpu;
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
pub mod hardware;

pub use crate::hardware::{Clock, Key, Stream, VRAM_HEIGHT, VRAM_WIDTH};
pub use crate::system::{Config, System};
