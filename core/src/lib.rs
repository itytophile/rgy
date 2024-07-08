//!
//! `rgy` is no-std cross-platform Rust GameBoy emulator library.
//!
//! The users of this library only needs to implement [`Hardware`][] trait, which abstracts OS-specific function.
//! Once it's implemented, the emulator works.
//!
//! The following code is the example which just implements `Hardware`. The implementation does nothing.
//! You can replace the body of each function with the actual meaningful logic.
//!
//! TODO rewrite the example

#![no_std]
// #![warn(missing_docs)]

mod alu;
mod cgb;
mod dma;
mod gpu;
mod high_ram;
mod ic;
mod joypad;
mod mbc;
mod oam;
mod serial;
pub mod sound;
pub mod system;
mod timer;

/// CPU state.
pub mod cpu;

/// Adaptor to register devices to MMU.
pub mod device;

/// Decoder which evaluates each CPU instructions.
pub mod inst;

/// Handles memory and I/O port access from the CPU.
pub mod mmu;

/// Hardware interface, which abstracts OS-specific functions.
mod hardware;

pub use crate::hardware::{Hardware, Key, Stream, VRAM_HEIGHT, VRAM_WIDTH};
pub use crate::system::System;
