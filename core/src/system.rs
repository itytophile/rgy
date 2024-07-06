use core::cell::RefCell;

use crate::cgb::Cgb;
use crate::cpu::Cpu;
use crate::debug::{Debugger, NullDebugger};
use crate::device::{Device, IoHandler, IoMemHandler};
use crate::dma::Dma;
use crate::fc::FreqControl;
use crate::gpu::Gpu;
use crate::hardware::{Hardware, HardwareHandle};
use crate::ic::{Ic, IcCells};
use crate::joypad::Joypad;
use crate::mbc::Mbc;
use crate::mmu::Mmu;
use crate::serial::Serial;
use crate::sound::{
    Mixer, MixerStream, NoiseStream, Sound, ToneStream, Unit, UnitRaw, Wave, WaveRaw, WaveStream,
};
use crate::timer::Timer;
use log::*;
use static_cell::StaticCell;

/// Configuration of the emulator.
pub struct Config {
    /// CPU frequency.
    pub(crate) freq: u64,
    /// Cycle sampling count in the CPU frequency controller.
    pub(crate) sample: u64,
    /// Delay unit in CPU frequency controller.
    pub(crate) delay_unit: u64,
    /// Don't adjust CPU frequency.
    pub(crate) native_speed: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self::new()
    }
}

impl Config {
    /// Create the default configuration.
    pub fn new() -> Self {
        let freq = 4194300; // 4.1943 MHz
        Self {
            freq,
            sample: freq / 1000,
            delay_unit: 10,
            native_speed: false,
        }
    }

    /// Set the CPU frequency.
    pub fn freq(mut self, freq: u64) -> Self {
        self.freq = freq;
        self
    }

    /// Set the sampling count of the CPU frequency controller.
    pub fn sample(mut self, sample: u64) -> Self {
        self.sample = sample;
        self
    }

    /// Set the delay unit.
    pub fn delay_unit(mut self, delay: u64) -> Self {
        self.delay_unit = delay;
        self
    }

    /// Set the flag to run at native speed.
    pub fn native_speed(mut self, native: bool) -> Self {
        self.native_speed = native;
        self
    }
}

/// Represents the entire emulator context.
pub struct System<'a, 'b, D> {
    cfg: Config,
    hw: HardwareHandle<'a>,
    fc: FreqControl<'a>,
    cpu: Cpu,
    mmu: Mmu<'a>,
    dbg: Device<'a, D>,
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
    pub fn new(
        rom: &'a [u8],
        hw: HardwareHandle<'a>,
        wave: Wave,
        mixer: Mixer,
        ic_cells: &'a IcCells,
    ) -> Self {
        let ic = Ic::new(ic_cells);
        let irq = ic.irq();
        Self {
            sound: RefCell::new(Sound::new(hw.clone(), wave, mixer)),
            ic: RefCell::new(ic),
            gpu: RefCell::new(Gpu::new(hw.clone(), irq.clone())),
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

impl<'a, 'b, D> System<'a, 'b, D>
where
    D: Debugger + 'static,
{
    /// Create a new emulator context.
    pub fn new(
        cfg: Config,
        hw_handle: HardwareHandle<'a>,
        dbg: &'a RefCell<D>,
        dbg_handler: &'a IoMemHandler<'a, D>,
        devices: Devices<'a, 'b>,
        handlers: &'a Handlers,
    ) -> Self {
        info!("Initializing...");

        let mut fc = FreqControl::new(hw_handle.clone(), &cfg);

        let dbg = Device::mediate(dbg);
        let cpu = Cpu::new();
        let mut mmu = Mmu::new();

        mmu.add_handler((0x0000, 0xffff), dbg_handler);

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
        mmu.add_handler((0xff40, 0xff55), &handlers.gpu);
        mmu.add_handler((0xff68, 0xff6b), &handlers.gpu);

        mmu.add_handler((0xff0f, 0xff0f), &handlers.ic);
        mmu.add_handler((0xffff, 0xffff), &handlers.ic);
        mmu.add_handler((0xff00, 0xff00), &handlers.joypad);
        mmu.add_handler((0xff04, 0xff07), &handlers.timer);
        mmu.add_handler((0xff01, 0xff02), &handlers.serial);

        dbg.borrow_mut().init(&mmu);

        info!("Starting...");

        fc.reset();

        Self {
            cfg,
            hw: hw_handle,
            fc,
            cpu,
            mmu,
            dbg,
            ic: devices.ic,
            gpu: devices.gpu,
            joypad: devices.joypad,
            timer: devices.timer,
            serial: devices.serial,
            dma: devices.dma,
        }
    }

    fn step(&mut self) -> PollState {
        {
            let mut dbg = self.dbg.borrow_mut();
            dbg.check_signal();
            dbg.take_cpu_snapshot(self.cpu.clone());
            dbg.on_decode(&self.mmu);
        }

        let mut time = self.cpu.execute(&mut self.mmu);

        time += self.cpu.check_interrupt(&mut self.mmu, &self.ic);

        self.dma.borrow_mut().step(&mut self.mmu);
        self.gpu.borrow_mut().step(time, &mut self.mmu);
        self.timer.borrow_mut().step(time);
        self.serial.borrow_mut().step(time);
        self.joypad.borrow_mut().poll();

        if !self.cfg.native_speed {
            self.fc.adjust(time);
            PollState {
                delay: self.fc.delay(),
            }
        } else {
            PollState { delay: 0 }
        }
    }

    /// Run a single step of emulation.
    /// This function needs to be called repeatedly until it returns `false`.
    /// Returning `false` indicates the end of emulation, and the functions shouldn't be called again.
    pub fn poll(&mut self) -> Option<PollState> {
        if !self.hw.get().borrow_mut().sched() {
            return None;
        }

        Some(self.step())
    }
}

pub struct PollState {
    pub delay: u64, // nano seconds
}

static TONE_UNIT1: StaticCell<UnitRaw<ToneStream>> = StaticCell::new();
static TONE_UNIT2: StaticCell<UnitRaw<ToneStream>> = StaticCell::new();
static WAVE_UNIT: StaticCell<UnitRaw<WaveStream>> = StaticCell::new();
static NOISE_UNIT: StaticCell<UnitRaw<NoiseStream>> = StaticCell::new();
static WAVE: StaticCell<WaveRaw> = StaticCell::new();

pub struct StackState0<D, H> {
    pub dbg_cell: RefCell<D>,
    pub hw_cell: RefCell<H>,
    pub ic_cells: IcCells,
}

pub fn get_stack_state0<H: Hardware + 'static, D: Debugger + 'static>(
    hw: H,
    dbg: D,
) -> StackState0<D, H> {
    StackState0 {
        dbg_cell: RefCell::new(dbg),
        hw_cell: RefCell::new(hw),
        ic_cells: IcCells::default(),
    }
}

pub struct StackState1<'a, D> {
    pub dbg_handler: IoMemHandler<'a, D>,
    pub hw_handle: HardwareHandle<'a>,
    pub raw_devices: RawDevices<'a>,
}

pub fn get_stack_state1<'a, D, H>(
    state0: &'a StackState0<D, H>,
    rom: &'a [u8],
) -> StackState1<'a, D>
where
    D: IoHandler,
    H: Hardware + 'static,
{
    let hw_handle = HardwareHandle::new(&state0.hw_cell);
    StackState1 {
        dbg_handler: Device::mediate(&state0.dbg_cell).handler(),
        hw_handle: hw_handle.clone(),
        raw_devices: RawDevices::new(
            rom,
            hw_handle,
            Wave::new(WAVE.init(WaveRaw::new())),
            Mixer::new(MixerStream::new(
                Unit::new(TONE_UNIT1.init(UnitRaw::default())),
                Unit::new(TONE_UNIT2.init(UnitRaw::default())),
                Unit::new(WAVE_UNIT.init(UnitRaw::default())),
                Unit::new(NOISE_UNIT.init(UnitRaw::default())),
            )),
            &state0.ic_cells,
        ),
    }
}
