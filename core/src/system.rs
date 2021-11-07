use crate::cpu::Cpu;
use crate::debug::Debugger;
use crate::fc::FreqControl;
use crate::hardware::{Hardware, HardwareHandle};
use crate::mmu::Mmu;
use log::*;

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
pub struct System<D> {
    cfg: Config,
    hw: HardwareHandle,
    fc: FreqControl,
    cpu: Cpu,
    dbg: D,
}

impl<D> System<D>
where
    D: Debugger + 'static,
{
    /// Create a new emulator context.
    pub fn new<T>(cfg: Config, rom: &[u8], hw: T, dbg: D) -> Self
    where
        T: Hardware + 'static,
    {
        info!("Initializing...");

        let hw = HardwareHandle::new(hw);

        let mut fc = FreqControl::new(hw.clone(), &cfg);

        let mmu = Mmu::new(hw.clone(), rom.to_vec());
        let cpu = Cpu::new(mmu);

        info!("Starting...");

        fc.reset();

        Self {
            cfg,
            hw,
            fc,
            cpu,
            dbg,
        }
    }

    /// Run a single step of emulation.
    /// This function needs to be called repeatedly until it returns `false`.
    /// Returning `false` indicates the end of emulation, and the functions shouldn't be called again.
    pub fn poll(&mut self) -> bool {
        if !self.hw.get().borrow_mut().sched() {
            return false;
        }

        let time = self.cpu.step();

        if !self.cfg.native_speed {
            self.fc.adjust(time);
        }

        true
    }
}

/// Run the emulator with the given configuration.
pub fn run<T: Hardware + 'static>(cfg: Config, rom: &[u8], hw: T) {
    run_inner(cfg, rom, hw, <dyn Debugger>::empty())
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

fn run_inner<T: Hardware + 'static, D: Debugger + 'static>(cfg: Config, rom: &[u8], hw: T, dbg: D) {
    let mut sys = System::new(cfg, rom, hw, dbg);
    while sys.poll() {}
}
