use crate::cpu::Cpu;
use crate::hardware::Hardware;
use crate::ic::Irq;
use crate::joypad::Joypad;
use crate::mbc::Mbc;
use crate::mmu::{MemHandlers, Mmu, MmuWithoutMixerStream};
use crate::serial::Serial;
use crate::sound::MixerStream;
use crate::VRAM_WIDTH;
use log::*;

/// Represents the entire emulator context.
pub struct System<'a> {
    pub cpu: Cpu,
    pub handlers: MemHandlers<'a>,
    pub mmu: MmuWithoutMixerStream,
}

impl<'a> System<'a> {
    /// Create a new emulator context.
    pub fn new(rom: &'a [u8], hw: &mut impl Hardware) -> Self {
        info!("Initializing...");

        let cpu = Cpu::new();
        let mmu = MmuWithoutMixerStream::default();

        info!("Starting...");

        Self {
            cpu,
            mmu,
            handlers: MemHandlers {
                ic: Default::default(),
                gpu: Default::default(),
                joypad: Joypad::new(),
                timer: Default::default(),
                serial: Serial::new(),
                dma: Default::default(),
                cgb: Default::default(),
                mbc: Mbc::new(rom, hw),
                sound: Default::default(),
            },
        }
    }

    fn step(
        &mut self,
        mixer_stream: &mut MixerStream,
        irq: &mut Irq,
        hw: &mut impl Hardware,
    ) -> PollState {
        let mut mmu = Mmu {
            inner: &mut self.mmu,
            mixer_stream,
            irq,
            handlers: &mut self.handlers,
            hw,
        };
        let mut time = self.cpu.execute(&mut mmu);

        time += self.cpu.check_interrupt(&mut mmu);

        mmu.dma_step();

        let line_to_draw = mmu.gpu_step(time);
        self.handlers.timer.step(time, irq);
        self.handlers.serial.step(time, irq, hw);
        self.handlers.joypad.poll(irq, hw);

        PollState { line_to_draw }
    }

    /// Run a single step of emulation.
    /// This function needs to be called repeatedly until it returns `false`.
    /// Returning `false` indicates the end of emulation, and the functions shouldn't be called again.
    pub fn poll(
        &mut self,
        mixer_stream: &mut MixerStream,
        irq: &mut Irq,
        hw: &mut impl Hardware,
    ) -> PollState {
        self.step(mixer_stream, irq, hw)
    }
}

pub struct PollState {
    pub line_to_draw: Option<(u8, [u32; VRAM_WIDTH])>,
}
