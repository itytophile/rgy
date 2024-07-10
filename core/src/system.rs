use crate::apu::mixer::MixerStream;
use crate::cpu::{Cpu, CpuState};
use crate::fc::FreqControl;
use crate::hardware::Hardware;
use crate::mmu::{Mmu, Peripherals};
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
    /// Emulate Gameboy Color
    pub(crate) color: bool,
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
            color: false,
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

    /// Set the flag to enable Gameboy Color.
    pub fn color(mut self, color: bool) -> Self {
        self.color = color;
        self
    }
}

/// Represents the entire emulator context.
pub struct System<'a, H: Hardware> {
    cfg: Config,
    fc: FreqControl,
    cpu_state: CpuState,
    peripherals: Peripherals<'a, H>,
}

impl<'a, H: Hardware + 'static> System<'a, H> {
    /// Create a new emulator context.
    pub fn new(cfg: Config, rom: &'a [u8], mut hw: H) -> Self {
        info!("Initializing...");

        let mut fc = FreqControl::new(&cfg);

        fc.reset(&mut hw);

        info!("Starting...");
        let peripherals = Peripherals::new(hw, rom, cfg.color);

        Self {
            cfg,
            fc,
            cpu_state: CpuState::new(),
            peripherals,
        }
    }

    /// Run a single step of emulation.
    /// This function needs to be called repeatedly until it returns `false`.
    /// Returning `false` indicates the end of emulation, and the functions shouldn't be called again.
    pub fn poll(&mut self, mixer_stream: &mut MixerStream) -> bool {
        if !self.peripherals.hw.sched() {
            return false;
        }

        let mut mmu = Mmu {
            mixer_stream,
            peripherals: &mut self.peripherals,
        };

        let mut cpu = Cpu {
            state: &mut self.cpu_state,
            sys: &mut mmu,
        };

        let time = cpu.execute();

        if !self.cfg.native_speed {
            self.fc.adjust(time, &mut self.peripherals.hw);
        }

        true
    }
}
