use core::cell::RefCell;

use crate::cgb::Cgb;
use crate::cpu::Cpu;
use crate::device::{Device, IoMemHandler};
use crate::dma::Dma;
use crate::gpu::Gpu;
use crate::hardware::{Hardware, HardwareHandle};
use crate::ic::{Ic, IcCells};
use crate::joypad::Joypad;
use crate::mbc::Mbc;
use crate::mmu::{Mmu, MmuWithoutMixerStream};
use crate::serial::Serial;
use crate::sound::{MixerStream, Sound};
use crate::timer::Timer;
use crate::VRAM_WIDTH;
use log::*;

/// Represents the entire emulator context.
pub struct System<'a, 'b> {
    hw: HardwareHandle<'a>,
    cpu: Cpu,
    mmu: MmuWithoutMixerStream<'a>,
    ic: Device<'a, Ic<'b>>,
    gpu: Device<'a, Gpu<'b>>,
    joypad: Device<'a, Joypad<'b>>,
    timer: Device<'a, Timer<'b>>,
    serial: Device<'a, Serial<'b>>,
    dma: Device<'a, Dma>,
}

pub struct RawDevices<'a> {
    sound: RefCell<Sound>,
    ic: RefCell<Ic<'a>>,
    gpu: RefCell<Gpu<'a>>,
    joypad: RefCell<Joypad<'a>>,
    timer: RefCell<Timer<'a>>,
    serial: RefCell<Serial<'a>>,
    mbc: RefCell<Mbc<'a>>,
    cgb: RefCell<Cgb>,
    dma: RefCell<Dma>,
}

impl<'a> RawDevices<'a> {
    pub fn new(rom: &'a [u8], hw: HardwareHandle<'a>, ic_cells: &'a IcCells) -> Self {
        let ic = Ic::new(ic_cells);
        let irq = ic.irq();
        Self {
            sound: RefCell::new(Sound::default()),
            ic: RefCell::new(ic),
            gpu: RefCell::new(Gpu::new(irq.clone())),
            joypad: RefCell::new(Joypad::new(hw.clone(), irq.clone())),
            timer: RefCell::new(Timer::new(irq.clone())),
            serial: RefCell::new(Serial::new(hw.clone(), irq.clone())),
            mbc: RefCell::new(Mbc::new(hw.clone(), rom)),
            cgb: RefCell::new(Cgb::new()),
            dma: RefCell::new(Dma::new()),
        }
    }
}

#[derive(Clone)]
pub struct Devices<'a, 'b> {
    sound: Device<'a, Sound>,
    ic: Device<'a, Ic<'b>>,
    gpu: Device<'a, Gpu<'b>>,
    joypad: Device<'a, Joypad<'b>>,
    timer: Device<'a, Timer<'b>>,
    serial: Device<'a, Serial<'b>>,
    mbc: Device<'a, Mbc<'b>>,
    cgb: Device<'a, Cgb>,
    dma: Device<'a, Dma>,
}

impl<'a, 'b> Devices<'a, 'b> {
    pub fn new(devices: &'a RawDevices<'b>) -> Self {
        let sound = Device::new(&devices.sound);
        let ic = Device::new(&devices.ic);
        let gpu = Device::new(&devices.gpu);
        let joypad = Device::new(&devices.joypad);
        let timer = Device::new(&devices.timer);
        let serial = Device::new(&devices.serial);
        let mbc = Device::new(&devices.mbc);
        let cgb = Device::new(&devices.cgb);
        let dma = Device::new(&devices.dma);
        Self {
            sound,
            ic,
            gpu,
            joypad,
            timer,
            serial,
            mbc,
            cgb,
            dma,
        }
    }
}

pub struct Handlers<'a, 'b> {
    sound: IoMemHandler<'a, Sound>,
    ic: IoMemHandler<'a, Ic<'b>>,
    gpu: IoMemHandler<'a, Gpu<'b>>,
    joypad: IoMemHandler<'a, Joypad<'b>>,
    timer: IoMemHandler<'a, Timer<'b>>,
    serial: IoMemHandler<'a, Serial<'b>>,
    mbc: IoMemHandler<'a, Mbc<'b>>,
    cgb: IoMemHandler<'a, Cgb>,
    dma: IoMemHandler<'a, Dma>,
}

impl<'a, 'b> Handlers<'a, 'b> {
    pub fn new(devices: Devices<'a, 'b>) -> Self {
        Self {
            sound: devices.sound.handler(),
            ic: devices.ic.handler(),
            gpu: devices.gpu.handler(),
            joypad: devices.joypad.handler(),
            timer: devices.timer.handler(),
            serial: devices.serial.handler(),
            mbc: devices.mbc.handler(),
            cgb: devices.cgb.handler(),
            dma: devices.dma.handler(),
        }
    }
}

impl<'a, 'b> System<'a, 'b> {
    /// Create a new emulator context.
    pub fn new(
        hw_handle: HardwareHandle<'a>,
        devices: Devices<'a, 'b>,
        handlers: &'a Handlers,
    ) -> Self {
        info!("Initializing...");

        let cpu = Cpu::new();
        let mut mmu = MmuWithoutMixerStream::default();

        // let ranges = [
        //     ((0xc000, 0xdfff)),
        //     ((0xff4d, 0xff4d)),
        //     ((0xff56, 0xff56)),
        //     ((0xff70, 0xff70)),
        //     ((0x0000, 0x7fff)),
        //     ((0xff50, 0xff50)),
        //     ((0xa000, 0xbfff)),
        //     ((0xff10, 0xff3f)),
        //     ((0xff46, 0xff46)),
        //     ((0x8000, 0x9fff)),
        //     ((0xff40, 0xff45)),
        //     ((0xff47, 0xff4b)),
        //     ((0xff4f, 0xff4f)),
        //     ((0xff51, 0xff55)),
        //     ((0xff68, 0xff6b)),
        //     ((0xff0f, 0xff0f)),
        //     ((0xffff, 0xffff)),
        //     ((0xff00, 0xff00)),
        //     ((0xff04, 0xff07)),
        //     ((0xff01, 0xff02)),
        // ];

        // for i in ranges {
        //     for j in ranges {
        //         if i != j && i.0 <= j.1 && j.0 <= i.1 {
        //             panic!("{:x?} {:x?}", i, j)
        //         }
        //     }
        // }

        mmu.add_handler((0xc000, 0xdfff), &handlers.cgb);
        mmu.add_handler((0xff4d, 0xff4d), &handlers.cgb);
        mmu.add_handler((0xff56, 0xff56), &handlers.cgb);
        mmu.add_handler((0xff70, 0xff70), &handlers.cgb);

        mmu.add_handler((0x0000, 0x7fff), &handlers.mbc);
        mmu.add_handler((0xff50, 0xff50), &handlers.mbc);
        mmu.add_handler((0xa000, 0xbfff), &handlers.mbc);
        mmu.add_handler((0xff10, 0xff3f), &handlers.sound);

        mmu.add_handler((0xff46, 0xff46), &handlers.dma);

        mmu.add_handler((0x8000, 0x9fff), &handlers.gpu);
        mmu.add_handler((0xff40, 0xff45), &handlers.gpu);
        mmu.add_handler((0xff47, 0xff4b), &handlers.gpu);
        mmu.add_handler((0xff4f, 0xff4f), &handlers.gpu);
        mmu.add_handler((0xff51, 0xff55), &handlers.gpu);
        mmu.add_handler((0xff68, 0xff6b), &handlers.gpu);

        mmu.add_handler((0xff0f, 0xff0f), &handlers.ic);
        mmu.add_handler((0xffff, 0xffff), &handlers.ic);
        mmu.add_handler((0xff00, 0xff00), &handlers.joypad);
        mmu.add_handler((0xff04, 0xff07), &handlers.timer);
        mmu.add_handler((0xff01, 0xff02), &handlers.serial);

        info!("Starting...");

        Self {
            hw: hw_handle,
            cpu,
            mmu,
            ic: devices.ic,
            gpu: devices.gpu,
            joypad: devices.joypad,
            timer: devices.timer,
            serial: devices.serial,
            dma: devices.dma,
        }
    }

    fn step(&mut self, mixer_stream: &mut MixerStream) -> PollState {
        let mut mmu = Mmu {
            inner: &mut self.mmu,
            mixer_stream,
        };
        let mut time = self.cpu.execute(&mut mmu);

        time += self.cpu.check_interrupt(&mut mmu, &self.ic);

        self.dma.borrow_mut().step(&mut mmu);
        let line_to_draw = self.gpu.borrow_mut().step(time, &mut mmu);
        self.timer.borrow_mut().step(time);
        self.serial.borrow_mut().step(time);
        self.joypad.borrow_mut().poll();

        PollState { line_to_draw }
    }

    /// Run a single step of emulation.
    /// This function needs to be called repeatedly until it returns `false`.
    /// Returning `false` indicates the end of emulation, and the functions shouldn't be called again.
    pub fn poll(&mut self, mixer_stream: &mut MixerStream) -> Option<PollState> {
        if !self.hw.get().borrow_mut().sched() {
            return None;
        }

        Some(self.step(mixer_stream))
    }
}

pub struct PollState {
    pub line_to_draw: Option<(u8, [u32; VRAM_WIDTH])>,
}

pub struct StackState0<H> {
    pub hw_cell: RefCell<H>,
    pub ic_cells: IcCells,
}

pub fn get_stack_state0<H: Hardware + 'static>(hw: H) -> StackState0<H> {
    StackState0 {
        hw_cell: RefCell::new(hw),
        ic_cells: IcCells::default(),
    }
}

pub struct StackState1<'a> {
    pub hw_handle: HardwareHandle<'a>,
    pub raw_devices: RawDevices<'a>,
}

pub fn get_stack_state1<'a, H>(state0: &'a StackState0<H>, rom: &'a [u8]) -> StackState1<'a>
where
    H: Hardware + 'static,
{
    let hw_handle = HardwareHandle::new(&state0.hw_cell);
    StackState1 {
        hw_handle: hw_handle.clone(),
        raw_devices: RawDevices::new(rom, hw_handle, &state0.ic_cells),
    }
}
