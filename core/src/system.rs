use core::cell::RefCell;

use crate::cgb::Cgb;
use crate::cpu::Cpu;
use crate::dma::Dma;
use crate::gpu::Gpu;
use crate::hardware::{Hardware, HardwareHandle};
use crate::ic::{Ic, Irq};
use crate::joypad::Joypad;
use crate::mbc::Mbc;
use crate::mmu::{Mmu, MmuWithoutMixerStream};
use crate::serial::Serial;
use crate::sound::{MixerStream, Sound};
use crate::timer::Timer;
use crate::VRAM_WIDTH;
use log::*;

/// Represents the entire emulator context.
pub struct System<'a> {
    pub hw: HardwareHandle<'a>,
    pub cpu: Cpu,
    pub mmu: MmuWithoutMixerStream,
    pub ic: Ic,
    pub gpu: Gpu,
    pub joypad: Joypad<'a>,
    pub timer: Timer,
    pub serial: Serial<'a>,
    pub dma: Dma,
    pub cgb: Cgb,
    pub mbc: Mbc<'a>,
    pub sound: Sound
}





impl<'a> System<'a> {
    /// Create a new emulator context.
    pub fn new(
        hw_handle: HardwareHandle<'a>,
        rom: &'a [u8]
    ) -> Self {
        info!("Initializing...");

        let cpu = Cpu::new();
        let mut mmu = MmuWithoutMixerStream::default();

        info!("Starting...");

        Self {
            hw: hw_handle.clone(),
            cpu,
            mmu,
            ic: Default::default(),
            gpu: Default::default(),
            joypad: Joypad::new(hw_handle.clone()),
            timer: Default::default(),
            serial: Serial::new(hw_handle.clone()),
            dma: Default::default(),
            cgb: Default::default(),
            mbc: Mbc::new(hw_handle, rom),
            sound: Default::default()
        }
    }

    fn step(&mut self, mixer_stream: &mut MixerStream, irq: &mut Irq) -> PollState {
        let mut mmu = Mmu {
            inner: &mut self.mmu,
            mixer_stream,
            irq,
        };
        let mut time = self.cpu.execute(&mut mmu);

        time += self.cpu.check_interrupt(&mut mmu, &self.ic);

        self.dma.step(&mut mmu);
        let line_to_draw = self.gpu.step(time, &mut mmu);
        self.timer.step(time, irq);
        self.serial.step(time, irq);
        self.joypad.poll(irq);

        PollState { line_to_draw }
    }

    /// Run a single step of emulation.
    /// This function needs to be called repeatedly until it returns `false`.
    /// Returning `false` indicates the end of emulation, and the functions shouldn't be called again.
    pub fn poll(&mut self, mixer_stream: &mut MixerStream, irq: &mut Irq) -> Option<PollState> {
        if !self.hw.get().borrow_mut().sched() {
            return None;
        }

        Some(self.step(mixer_stream, irq))
    }
}

pub struct PollState {
    pub line_to_draw: Option<(u8, [u32; VRAM_WIDTH])>,
}

pub struct StackState0<H> {
    pub hw_cell: RefCell<H>,
}

pub fn get_stack_state0<H: Hardware + 'static>(hw: H) -> StackState0<H> {
    StackState0 {
        hw_cell: RefCell::new(hw),
    }
}

pub struct StackState1<'a> {
    pub hw_handle: HardwareHandle<'a>,
}

pub fn get_stack_state1<'a, H>(state0: &'a StackState0<H>) -> StackState1<'a>
where
    H: Hardware + 'static,
{
    let hw_handle = HardwareHandle::new(&state0.hw_cell);
    StackState1 {
        hw_handle: hw_handle.clone(),
    }
}
