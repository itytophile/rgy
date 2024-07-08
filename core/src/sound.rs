use log::*;

use crate::device::IoHandler;
use crate::hardware::Stream;
use crate::ic::Irq;
use crate::mmu::{MemRead, MemWrite};
use crate::Hardware;

struct Sweep {
    enable: bool,
    freq: usize,
    time: usize,
    sub: bool,
    shift: usize,
    clock: usize,
}

impl Sweep {
    fn new(enable: bool, freq: usize, time: usize, sub: bool, shift: usize) -> Self {
        Self {
            enable,
            freq,
            time,
            sub,
            shift,
            clock: 0,
        }
    }

    fn freq(&mut self, rate: usize) -> usize {
        if !self.enable || self.time == 0 || self.shift == 0 {
            return self.freq;
        }

        let interval = rate * self.time / 128;

        self.clock += 1;
        if self.clock >= interval {
            self.clock -= interval;

            let p = self.freq / 2usize.pow(self.shift as u32);

            self.freq = if self.sub {
                self.freq.saturating_sub(p)
            } else {
                self.freq.saturating_add(p)
            };

            if self.freq >= 2048 || self.freq == 0 {
                self.enable = false;
                self.freq = 0;
            }
        }

        self.freq
    }
}

struct Envelop {
    amp: usize,
    count: usize,
    inc: bool,
    clock: usize,
}

impl Envelop {
    fn new(amp: usize, count: usize, inc: bool) -> Self {
        Self {
            amp,
            count,
            inc,
            clock: 0,
        }
    }

    fn amp(&mut self, rate: usize) -> usize {
        if self.amp == 0 {
            return 0;
        }

        if self.count == 0 {
            return self.amp;
        }

        let interval = rate * self.count / 64;

        self.clock += 1;
        if self.clock >= interval {
            self.clock -= interval;

            self.amp = if self.inc {
                self.amp.saturating_add(1).min(15)
            } else {
                self.amp.saturating_sub(1)
            };
        }

        self.amp
    }
}

struct Counter {
    enable: bool,
    count: usize,
    base: usize,
    clock: usize,
}

impl Counter {
    fn new(enable: bool, count: usize, base: usize) -> Self {
        Self {
            enable,
            count,
            base,
            clock: 0,
        }
    }

    fn stop(&mut self, rate: usize) -> bool {
        if !self.enable {
            return false;
        }

        let deadline = rate * (self.base - self.count) / 256;

        if self.clock >= deadline {
            true
        } else {
            self.clock += 1;
            false
        }
    }
}

struct WaveIndex {
    clock: usize,
    index: usize,
}

impl WaveIndex {
    fn new() -> Self {
        Self { clock: 0, index: 0 }
    }

    fn index(&mut self, rate: usize, freq: usize, max: usize) -> usize {
        self.clock += freq;

        if self.clock >= rate {
            self.clock -= rate;
            self.index = (self.index + 1) % max;
        }

        self.index
    }
}

#[allow(clippy::upper_case_acronyms)]
struct LFSR {
    value: u16,
    short: bool,
}

impl LFSR {
    fn new(short: bool) -> Self {
        Self {
            value: 0xdead,
            short,
        }
    }

    fn high(&self) -> bool {
        self.value & 1 == 0
    }

    fn update(&mut self) {
        if self.short {
            self.value &= 0xff;
            let bit = (self.value & 0x0001)
                ^ ((self.value & 0x0004) >> 2)
                ^ ((self.value & 0x0008) >> 3)
                ^ ((self.value & 0x0010) >> 5);
            self.value = (self.value >> 1) | (bit << 7);
        } else {
            let bit = (self.value & 0x0001)
                ^ ((self.value & 0x0004) >> 2)
                ^ ((self.value & 0x0008) >> 3)
                ^ ((self.value & 0x0020) >> 5);
            self.value = (self.value >> 1) | (bit << 15);
        }
    }
}

struct RandomWave {
    lfsr: LFSR,
    clock: usize,
}

impl RandomWave {
    fn new(short: bool) -> Self {
        Self {
            lfsr: LFSR::new(short),
            clock: 0,
        }
    }

    fn high(&mut self, rate: usize, freq: usize) -> bool {
        self.clock += freq;

        if self.clock >= rate {
            self.clock -= rate;
            self.lfsr.update()
        }

        self.lfsr.high()
    }
}

#[derive(Debug, Clone, Default)]
struct Tone {
    sweep_time: usize,
    sweep_sub: bool,
    sweep_shift: usize,
    sound_len: usize,
    wave_duty: usize,
    env_init: usize,
    env_inc: bool,
    env_count: usize,
    counter: bool,
    freq: usize,
}

impl Tone {
    fn on_read(&mut self, base: u16, addr: u16) -> MemRead {
        if addr == base + 3 {
            MemRead(0xff)
        } else {
            unreachable!()
        }
    }

    fn on_write(&mut self, base: u16, addr: u16, value: u8) -> bool {
        if addr == base {
            self.sweep_time = ((value >> 4) & 0x7) as usize;
            self.sweep_sub = value & 0x08 != 0;
            self.sweep_shift = (value & 0x07) as usize;
        } else if addr == base + 1 {
            self.wave_duty = (value >> 6).into();
            self.sound_len = (value & 0x1f) as usize;
        } else if addr == base + 2 {
            self.env_init = (value >> 4) as usize;
            self.env_inc = value & 0x08 != 0;
            self.env_count = (value & 0x7) as usize;
        } else if addr == base + 3 {
            self.freq = (self.freq & !0xff) | value as usize;
        } else if addr == base + 4 {
            self.counter = value & 0x40 != 0;
            self.freq = (self.freq & !0x700) | (((value & 0x7) as usize) << 8);
            return value & 0x80 != 0;
        } else {
            unreachable!()
        }

        false
    }
}

pub struct ToneStream {
    tone: Tone,
    sweep: Sweep,
    env: Envelop,
    counter: Counter,
    index: WaveIndex,
}

impl ToneStream {
    fn new(tone: Tone, sweep: bool) -> Self {
        let freq = 131072 / (2048 - tone.freq);
        let sweep = Sweep::new(
            sweep,
            freq,
            tone.sweep_time,
            tone.sweep_sub,
            tone.sweep_shift,
        );
        let env = Envelop::new(tone.env_init, tone.env_count, tone.env_inc);
        let counter = Counter::new(tone.counter, tone.sound_len, 64);

        Self {
            tone,
            sweep,
            env,
            counter,
            index: WaveIndex::new(),
        }
    }
}

impl Stream for ToneStream {
    fn next(&mut self, rate: u32) -> u16 {
        let rate = rate as usize;

        // Stop counter
        if self.counter.stop(rate) {
            return 0;
        }

        // Envelop
        let amp = self.env.amp(rate);

        // Sweep
        let freq = self.sweep.freq(rate);

        // Square wave generation
        let duty = match self.tone.wave_duty {
            0 => 0,
            1 => 1,
            2 => 3,
            3 => 5,
            _ => unreachable!(),
        };

        let index = self.index.index(rate, freq * 8, 8);
        if index <= duty {
            0
        } else {
            amp as u16
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Wave {
    enable: bool,
    sound_len: usize,
    amp_shift: usize,
    counter: bool,
    freq: usize,
    wavebuf: [u8; 16],
}

impl Wave {
    fn on_read(&mut self, addr: u16) -> MemRead {
        if addr == 0xff1d {
            MemRead(0xff)
        } else {
            unreachable!()
        }
    }

    fn on_write(&mut self, addr: u16, value: u8) -> bool {
        if addr == 0xff1a {
            debug!("Wave enable: {:02x}", value);
            self.enable = value & 0x80 != 0;
            return true;
        } else if addr == 0xff1b {
            debug!("Wave len: {:02x}", value);
            self.sound_len = value as usize;
        } else if addr == 0xff1c {
            debug!("Wave amp shift: {:02x}", value);
            self.amp_shift = (value as usize >> 5) & 0x3;
        } else if addr == 0xff1d {
            debug!("Wave freq1: {:02x}", value);
            self.freq = (self.freq & !0xff) | value as usize;
        } else if addr == 0xff1e {
            debug!("Wave freq2: {:02x}", value);
            self.counter = value & 0x40 != 0;
            self.freq = (self.freq & !0x700) | (((value & 0x7) as usize) << 8);
            return value & 0x80 != 0;
        } else if (0xff30..=0xff3f).contains(&addr) {
            self.wavebuf[(addr - 0xff30) as usize] = value;
        } else {
            unreachable!()
        }

        false
    }
}

pub struct WaveStream {
    wave: Wave,
    counter: Counter,
    index: WaveIndex,
}

impl WaveStream {
    fn new(wave: Wave) -> Self {
        let counter = Counter::new(wave.counter, wave.sound_len, 256);

        Self {
            wave,
            counter,
            index: WaveIndex::new(),
        }
    }
}

impl Stream for WaveStream {
    fn next(&mut self, rate: u32) -> u16 {
        if !self.wave.enable {
            return 0;
        }

        let rate = rate as usize;

        // Stop counter
        if self.counter.stop(rate) {
            return 0;
        }

        let samples = self.wave.wavebuf.len() * 2;
        let freq = 65536 / (2048 - self.wave.freq);
        let index_freq = freq * samples;
        let index = self.index.index(rate, index_freq, samples);

        let amp = if index % 2 == 0 {
            self.wave.wavebuf[index / 2] >> 4
        } else {
            self.wave.wavebuf[index / 2] & 0xf
        };

        let amp = match self.wave.amp_shift {
            0 => 0,
            1 => amp,
            2 => amp >> 1,
            3 => amp >> 2,
            _ => unreachable!(),
        };

        amp as u16
    }
}

#[derive(Debug, Clone, Default)]
struct Noise {
    sound_len: usize,

    env_init: usize,
    env_inc: bool,
    env_count: usize,

    shift_freq: usize,
    step: bool,
    div_freq: usize,

    counter: bool,
}

impl Noise {
    fn on_read(&mut self, _addr: u16) -> MemRead {
        unreachable!()
    }

    fn on_write(&mut self, addr: u16, value: u8) -> bool {
        if addr == 0xff20 {
            self.sound_len = (value & 0x1f) as usize;
        } else if addr == 0xff21 {
            self.env_init = (value >> 4) as usize;
            self.env_inc = value & 0x08 != 0;
            self.env_count = (value & 0x7) as usize;
        } else if addr == 0xff22 {
            self.shift_freq = (value >> 4) as usize;
            self.step = value & 0x08 != 0;
            self.div_freq = (value & 0x7) as usize;
        } else if addr == 0xff23 {
            self.counter = value & 0x40 != 0;
            return value & 0x80 != 0;
        } else {
            unreachable!()
        }

        false
    }
}

pub struct NoiseStream {
    noise: Noise,
    env: Envelop,
    counter: Counter,
    wave: RandomWave,
}

impl NoiseStream {
    fn new(noise: Noise) -> Self {
        let env = Envelop::new(noise.env_init, noise.env_count, noise.env_inc);
        let counter = Counter::new(noise.counter, noise.sound_len, 64);
        let wave = RandomWave::new(noise.step);

        Self {
            noise,
            env,
            counter,
            wave,
        }
    }
}

impl Stream for NoiseStream {
    fn next(&mut self, rate: u32) -> u16 {
        let rate = rate as usize;

        // Stop counter
        if self.counter.stop(rate) {
            return 0;
        }

        // Envelop
        let amp = self.env.amp(rate);

        // Noise: 524288 Hz / r / 2 ^ (s+1)
        let r = self.noise.div_freq;
        let s = self.noise.shift_freq as u32;
        let freq = if r == 0 {
            // For r = 0, assume r = 0.5 instead
            524288 * 5 / 10 / 2usize.pow(s + 1)
        } else {
            524288 / self.noise.div_freq / 2usize.pow(s + 1)
        };

        if self.wave.high(rate, freq) {
            amp as u16
        } else {
            0
        }
    }
}

#[derive(Default)]
pub struct Mixer {
    so1_volume: usize,
    so2_volume: usize,
    so_mask: u8,
    enable: bool,
}

impl Mixer {
    fn on_read(&mut self, addr: u16, stream: &MixerStream) -> MemRead {
        // https://gbdev.gg8.se/wiki/articles/Gameboy_sound_hardware#Register_Reading
        match addr {
            0xff25 => MemRead(self.so_mask),
            0xff26 => {
                let mut v = 0;
                v |= if self.enable { 0x80 } else { 0x00 };
                v |= if stream.tone1.on() { 0x08 } else { 0x00 };
                v |= if stream.tone2.on() { 0x04 } else { 0x00 };
                v |= if stream.wave.on() { 0x02 } else { 0x00 };
                v |= if stream.noise.on() { 0x01 } else { 0x00 };
                MemRead(v)
            }
            _ => unreachable!("{:x}", addr),
        }
    }

    fn on_write(&mut self, addr: u16, value: u8, stream: &mut MixerStream) {
        if addr == 0xff24 {
            self.so1_volume = (value as usize & 0x70) >> 4;
            self.so2_volume = value as usize & 0x07;
            self.update_volume(stream);
        } else if addr == 0xff25 {
            self.so_mask = value;
            self.update_volume(stream);
        } else if addr == 0xff26 {
            self.enable = value & 0x80 != 0;
            if self.enable {
                info!("Sound master enabled");
            } else {
                info!("Sound master disabled");
            }
            self.update_volume(stream);
        } else {
            unreachable!()
        }
    }

    fn restart_tone1(&mut self, t: Tone, stream: &mut MixerStream) {
        stream.tone1.update(Some(ToneStream::new(t, true)));
    }

    fn restart_tone2(&mut self, t: Tone, stream: &mut MixerStream) {
        stream.tone2.update(Some(ToneStream::new(t, false)));
    }

    fn restart_wave(&mut self, w: Wave, stream: &mut MixerStream) {
        stream.wave.update(Some(WaveStream::new(w)));
    }

    fn restart_noise(&mut self, n: Noise, stream: &mut MixerStream) {
        stream.noise.update(Some(NoiseStream::new(n)));
    }

    fn update_volume(&mut self, stream: &mut MixerStream) {
        stream.tone1.volume = self.get_volume(0);
        stream.tone2.volume = self.get_volume(1);
        stream.wave.volume = self.get_volume(2);
        stream.noise.volume = self.get_volume(3);
    }

    fn get_volume(&self, id: u8) -> usize {
        let mask: usize = 1 << id;
        let v1 = if usize::from(self.so_mask) & mask != 0 {
            self.so1_volume
        } else {
            0
        };
        let v2 = if usize::from(self.so_mask) & (mask << 4) != 0 {
            self.so2_volume
        } else {
            0
        };
        v1 + v2
    }
}

pub struct Unit<T> {
    stream: Option<T>,
    volume: usize,
}

impl<T> Default for Unit<T> {
    fn default() -> Self {
        Self {
            stream: Default::default(),
            volume: Default::default(),
        }
    }
}

impl<T: Stream> Unit<T> {
    fn on(&self) -> bool {
        self.stream.is_some()
    }

    fn update(&mut self, s: Option<T>) {
        self.stream = s;
    }

    fn next(&mut self, rate: u32) -> (u16, u16) {
        (
            self.stream.as_mut().map(|s| s.next(rate)).unwrap_or(0),
            self.volume as u16,
        )
    }
}

#[derive(Default)]
pub struct MixerStream {
    tone1: Unit<ToneStream>,
    tone2: Unit<ToneStream>,
    wave: Unit<WaveStream>,
    noise: Unit<NoiseStream>,
}

impl MixerStream {
    fn volume(&self, amp: u16, vol: u16) -> u16 {
        amp * vol
    }

    pub const MAX: u16 = 840 * 3;
}

impl Stream for MixerStream {
    fn next(&mut self, rate: u32) -> u16 {
        let mut vol = 0;

        let (t, v) = self.tone1.next(rate);
        vol += self.volume(t, v);
        let (t, v) = self.tone2.next(rate);
        vol += self.volume(t, v);
        let (t, v) = self.wave.next(rate);
        vol += self.volume(t, v);
        let (t, v) = self.noise.next(rate);
        vol += self.volume(t, v) / 2; // Soften the noise

        assert!(vol <= 840, "vol = {}", vol);

        vol
    }
}

#[derive(Default)]
pub struct Sound {
    tone1: Tone,
    tone2: Tone,
    wave: Wave,
    noise: Noise,
    mixer: Mixer,
}

impl IoHandler for Sound {
    fn on_read(
        &mut self,
        addr: u16,
        mixer_stream: &MixerStream,
        _: &Irq,
        _: &mut impl Hardware,
    ) -> MemRead {
        if (0xff10..=0xff14).contains(&addr) {
            self.tone1.on_read(0xff10, addr)
        } else if (0xff15..=0xff19).contains(&addr) {
            self.tone2.on_read(0xff15, addr)
        } else if (0xff1a..=0xff1e).contains(&addr) {
            self.wave.on_read(addr)
        } else if (0xff20..=0xff23).contains(&addr) {
            self.noise.on_read(addr)
        } else if (0xff24..=0xff26).contains(&addr) {
            self.mixer.on_read(addr, mixer_stream)
        } else {
            unreachable!()
        }
    }

    fn on_write(
        &mut self,
        addr: u16,
        value: u8,
        mixer_stream: &mut MixerStream,
        _: &mut Irq,
        _: &mut impl Hardware,
    ) -> MemWrite {
        if (0xff10..=0xff14).contains(&addr) {
            if self.tone1.on_write(0xff10, addr, value) {
                self.mixer.restart_tone1(self.tone1.clone(), mixer_stream);
            }
        } else if (0xff15..=0xff19).contains(&addr) {
            if self.tone2.on_write(0xff15, addr, value) {
                self.mixer.restart_tone2(self.tone2.clone(), mixer_stream);
            }
        } else if (0xff1a..=0xff1e).contains(&addr) {
            if self.wave.on_write(addr, value) {
                self.mixer.restart_wave(self.wave.clone(), mixer_stream);
            }
        } else if (0xff30..=0xff3f).contains(&addr) {
            let _ = self.wave.on_write(addr, value);
        } else if (0xff20..=0xff23).contains(&addr) {
            if self.noise.on_write(addr, value) {
                self.mixer.restart_noise(self.noise.clone(), mixer_stream);
            }
        } else if (0xff24..=0xff26).contains(&addr) {
            self.mixer.on_write(addr, value, mixer_stream);
        } else {
            info!("Write sound: {:04x} {:02x}", addr, value);
        }

        MemWrite::PassThrough
    }
}
