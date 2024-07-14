use crate::apu::mixer::MixerStream;
use crate::cpu::{Cpu, CpuState};
use crate::hardware::{Hardware, JoypadInput};
use crate::mmu::{GameboyMode, Mmu, Peripherals};
use crate::{gpu, VRAM_WIDTH};

/// Configuration of the emulator.
pub struct Config {
    /// CPU frequency.
    pub(crate) freq: u64,
    /// Cycle sampling count in the CPU frequency controller.
    pub(crate) sample: u64,
    /// Delay unit in CPU frequency controller.
    pub(crate) delay_unit: u64,
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

    /// Set the flag to enable Gameboy Color.
    pub fn color(mut self, color: bool) -> Self {
        self.color = color;
        self
    }
}

/// Represents the entire emulator context.
pub struct System<'a, H: Hardware, GB: GameboyMode> {
    cpu_state: CpuState<GB>,
    peripherals: Peripherals<'a, H, GB>,
}

impl<'a, H: Hardware + 'static, GB: GameboyMode> System<'a, H, GB> {
    /// Create a new emulator context.
    pub fn new(cfg: Config, rom: &'a [u8], hw: H, cartridge_ram: &'a mut [u8]) -> Self {
        let peripherals = Peripherals::new(hw, rom, cfg.color, cartridge_ram);

        Self {
            cpu_state: CpuState::new(),
            peripherals,
        }
    }

    /// Run a single step of emulation.
    /// This function needs to be called repeatedly.
    /// You have to check if the seria_input has been consumed or not (from Some(_) to None)
    pub fn poll(
        &mut self,
        mixer_stream: &mut MixerStream,
        joypad_input: JoypadInput,
        serial_input: &mut Option<u8>,
    ) -> PollData<<GB::Gpu as gpu::CgbExt>::Color> {
        // the serial peripheral can send bytes during the serial step or after a write to some memory from the CPU
        self.peripherals.serial.clear_sent_bytes();

        let mut mmu = Mmu {
            mixer_stream,
            peripherals: &mut self.peripherals,
            joypad_input,
            serial_input,
        };

        let mut cpu = Cpu {
            state: &mut self.cpu_state,
            sys: &mut mmu,
        };

        let time = cpu.execute();

        PollData {
            line_to_draw: self
                .cpu_state
                .steps_data
                .line_to_draw
                .as_ref()
                .map(|line_to_draw| (line_to_draw.0, &line_to_draw.1)),
            cpu_time: time,
            serial_sent_bytes: self.peripherals.serial.get_sent_bytes(),
        }
    }
}

pub struct PollData<'a, C> {
    pub line_to_draw: Option<(u8, &'a [C; VRAM_WIDTH])>,
    pub cpu_time: usize,
    pub serial_sent_bytes: &'a [u8],
}
