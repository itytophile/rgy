use core::cell::RefCell;

use crate::cgb::Cgb;
use crate::cpu::Cpu;
use crate::debug::{Debugger, NullDebugger};
use crate::device::{Device, IoMemHandler};
use crate::dma::Dma;
use crate::fc::FreqControl;
use crate::gpu::Gpu;
use crate::hardware::{Hardware, HardwareHandle};
use crate::ic::Ic;
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
pub struct System<'a, D> {
    cfg: Config,
    hw: HardwareHandle,
    fc: FreqControl,
    cpu: Cpu,
    mmu: Option<Mmu<'a>>,
    dbg: Device<'a, D>,
    ic: Device<'a, Ic>,
    gpu: Device<'a, Gpu>,
    joypad: Device<'a, Joypad>,
    timer: Device<'a, Timer>,
    serial: Device<'a, Serial>,
    dma: Device<'a, Dma>,
}

struct RawDevices {
    sound: RefCell<Sound>,
    ic: RefCell<Ic>,
    gpu: RefCell<Gpu>,
    joypad: RefCell<Joypad>,
    timer: RefCell<Timer>,
    serial: RefCell<Serial>,
    mbc: RefCell<Mbc>,
    cgb: RefCell<Cgb>,
    dma: RefCell<Dma>,
}

impl RawDevices {
    pub fn new(rom: &[u8], hw: HardwareHandle, wave: Wave, mixer: Mixer) -> Self {
        let ic = Ic::new();
        let irq = ic.irq();
        Self {
            sound: RefCell::new(Sound::new(hw.clone(), wave, mixer)),
            ic: RefCell::new(ic),
            gpu: RefCell::new(Gpu::new(hw.clone(), irq.clone())),
            joypad: RefCell::new(Joypad::new(hw.clone(), irq.clone())),
            timer: RefCell::new(Timer::new(irq.clone())),
            serial: RefCell::new(Serial::new(hw.clone(), irq.clone())),
            mbc: RefCell::new(Mbc::new(hw.clone(), rom.to_vec())),
            cgb: RefCell::new(Cgb::new()),
            dma: RefCell::new(Dma::new()),
        }
    }
}

#[derive(Clone)]
pub struct Devices<'a> {
    sound: Device<'a, Sound>,
    ic: Device<'a, Ic>,
    gpu: Device<'a, Gpu>,
    joypad: Device<'a, Joypad>,
    timer: Device<'a, Timer>,
    serial: Device<'a, Serial>,
    mbc: Device<'a, Mbc>,
    cgb: Device<'a, Cgb>,
    dma: Device<'a, Dma>,
}

impl<'a> Devices<'a> {
    fn new(devices: &'a RawDevices) -> Self {
        let sound = Device::new(&devices.sound);
        let ic = Device::new(&devices.ic);
        let gpu = Device::new(&devices.gpu);
        let joypad = Device::new(&devices.joypad);
        let timer = Device::new(&devices.timer);
        let serial = Device::new(&devices.serial);
        let mbc = Device::new(&devices.mbc);
        let cgb = Device::new(&devices.cgb);
        let dma = Device::new(&devices.dma);
        Devices {
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

pub struct Handlers<'a> {
    sound: IoMemHandler<'a, Sound>,
    ic: IoMemHandler<'a, Ic>,
    gpu: IoMemHandler<'a, Gpu>,
    joypad: IoMemHandler<'a, Joypad>,
    timer: IoMemHandler<'a, Timer>,
    serial: IoMemHandler<'a, Serial>,
    mbc: IoMemHandler<'a, Mbc>,
    cgb: IoMemHandler<'a, Cgb>,
    dma: IoMemHandler<'a, Dma>,
}

impl<'a> Handlers<'a> {
    fn new(devices: Devices<'a>) -> Self {
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

impl<'a, D> System<'a, D>
where
    D: Debugger + 'static,
{
    /// Create a new emulator context.
    pub fn new(
        cfg: Config,
        hw_handle: HardwareHandle,
        dbg: &'a RefCell<D>,
        dbg_handler: &'a IoMemHandler<'a, D>,
        devices: Devices<'a>,
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

        let mmu = Some(mmu);

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

    fn step(&mut self, mut mmu: Mmu<'a>) -> Mmu<'a> {
        {
            let mut dbg = self.dbg.borrow_mut();
            dbg.check_signal();
            dbg.take_cpu_snapshot(self.cpu.clone());
            dbg.on_decode(&mmu);
        }

        let mut time = self.cpu.execute(&mut mmu);

        time += self.cpu.check_interrupt(&mut mmu, &self.ic);

        self.dma.borrow_mut().step(&mut mmu);
        self.gpu.borrow_mut().step(time, &mut mmu);
        self.timer.borrow_mut().step(time);
        self.serial.borrow_mut().step(time);
        self.joypad.borrow_mut().poll();

        if !self.cfg.native_speed {
            self.fc.adjust(time);
        }

        mmu
    }

    /// Run a single step of emulation.
    /// This function needs to be called repeatedly until it returns `false`.
    /// Returning `false` indicates the end of emulation, and the functions shouldn't be called again.
    pub fn poll(&mut self) -> bool {
        if !self.hw.get().borrow_mut().sched() {
            return false;
        }

        let mmu = self.mmu.take().unwrap();
        self.mmu = Some(self.step(mmu));

        true
    }
}

/// Run the emulator with the given configuration.
pub fn run<T: Hardware + 'static>(cfg: Config, rom: &[u8], hw: T) {
    run_inner(cfg, rom, hw, NullDebugger)
}

/// Run the emulator with the given configuration and debugger.
pub fn run_debug<T: Hardware + 'static, D: Debugger + 'static>(
    cfg: Config,
    rom: &[u8],
    hw: T,
    dbg: D,
) {
    run_inner(cfg, rom, hw, dbg)
}

static TONE_UNIT1: StaticCell<UnitRaw<ToneStream>> = StaticCell::new();
static TONE_UNIT2: StaticCell<UnitRaw<ToneStream>> = StaticCell::new();
static WAVE_UNIT: StaticCell<UnitRaw<WaveStream>> = StaticCell::new();
static NOISE_UNIT: StaticCell<UnitRaw<NoiseStream>> = StaticCell::new();
static WAVE: StaticCell<WaveRaw> = StaticCell::new();

fn run_inner<T: Hardware + 'static, D: Debugger + 'static>(cfg: Config, rom: &[u8], hw: T, dbg: D) {
    let dbg_cell = &RefCell::new(dbg);
    let dbg = Device::mediate(dbg_cell);
    let hw_handle = HardwareHandle::new(hw);
    let raw_devices = RawDevices::new(
        rom,
        hw_handle.clone(),
        Wave::new(WAVE.init(WaveRaw::new())),
        Mixer::new(MixerStream::new(
            Unit::new(TONE_UNIT1.init(UnitRaw::default())),
            Unit::new(TONE_UNIT2.init(UnitRaw::default())),
            Unit::new(WAVE_UNIT.init(UnitRaw::default())),
            Unit::new(NOISE_UNIT.init(UnitRaw::default())),
        )),
    );
    let devices = Devices::new(&raw_devices);
    let dbg_handle = dbg.handler();
    let handlers = Handlers::new(devices.clone());
    let mut sys = System::new(cfg, hw_handle, dbg_cell, &dbg_handle, devices, &handlers);
    while sys.poll() {}
}
