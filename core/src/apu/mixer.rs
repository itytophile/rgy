use super::{
    noise::{Noise, NoiseStream},
    tone::{Tone, ToneStream},
    wave::{Wave, WaveStream},
};
use crate::hardware::Stream;

pub struct Mixer {
    ctrl: u8,
    so1_volume: usize,
    so2_volume: usize,
    so_mask: usize,
    enable: bool,
}

impl Mixer {
    pub fn new() -> Self {
        Self {
            ctrl: 0,
            so1_volume: 0,
            so2_volume: 0,
            so_mask: 0,
            enable: false,
        }
    }

    /// Read NR50 register (0xff24)
    pub fn read_ctrl(&self) -> u8 {
        self.ctrl
    }

    /// Write NR50 register (0xff24)
    pub fn write_ctrl(&mut self, value: u8, stream: &mut MixerStream) {
        self.ctrl = value;
        self.so1_volume = (value as usize & 0x70) >> 4;
        self.so2_volume = value as usize & 0x07;
        self.update_stream(stream);
    }

    /// Read NR51 register (0xff25)
    pub fn read_so_mask(&self) -> u8 {
        self.so_mask as u8
    }

    /// Write NR51 register (0xff25)
    pub fn write_so_mask(&mut self, value: u8, stream: &mut MixerStream) {
        self.so_mask = value as usize;
        self.update_stream(stream);
    }

    pub fn sync_tone(&mut self, index: usize, tone: Tone, stream: &mut MixerStream) {
        stream.tones[index].update(Some(tone.create_stream()));
    }

    pub fn sync_wave(&mut self, wave: Wave, stream: &mut MixerStream) {
        stream.wave.update(Some(wave.create_stream()));
    }

    pub fn sync_noise(&mut self, noise: Noise, stream: &mut MixerStream) {
        stream.noise.update(Some(noise.create_stream()));
    }

    pub fn step(&mut self, _cycles: usize) {}

    pub fn enable(&mut self, enable: bool, stream: &mut MixerStream) {
        self.enable = enable;
        self.update_stream(stream);
    }

    // Update streams based on register settings
    fn update_stream(&mut self, stream: &mut MixerStream) {
        stream.enable = self.enable;

        if self.enable {
            for (i, tone) in stream.tones.iter_mut().enumerate() {
                tone.volume =
                    Self::get_volume(i as u8, self.so_mask, self.so1_volume, self.so2_volume);
            }
            stream.wave.volume =
                Self::get_volume(2, self.so_mask, self.so1_volume, self.so2_volume);
            stream.noise.volume =
                Self::get_volume(3, self.so_mask, self.so1_volume, self.so2_volume);
        }
    }

    fn get_volume(id: u8, so_mask: usize, so1_volume: usize, so2_volume: usize) -> usize {
        let mask = 1 << id;
        let v1 = if so_mask & mask != 0 { so1_volume } else { 0 };
        let v2 = if so_mask & (mask << 4) != 0 {
            so2_volume
        } else {
            0
        };
        v1 + v2
    }

    pub fn clear(&mut self, stream: &mut MixerStream) {
        self.ctrl = 0;
        self.so1_volume = 0;
        self.so2_volume = 0;
        self.so_mask = 0;
        for tone in &mut stream.tones {
            tone.clear();
        }
        stream.wave.clear();
        stream.noise.clear();
    }
}

struct Unit<T> {
    stream: Option<T>,
    volume: usize,
}

impl<T> Unit<T> {
    fn new() -> Self {
        Self {
            stream: None,
            volume: 0,
        }
    }
}

impl<T: Stream> Unit<T> {
    fn update(&mut self, s: Option<T>) {
        self.stream = s;
    }

    fn clear(&mut self) {
        self.update(None);
    }

    fn next(&mut self, rate: u32) -> (u16, u16) {
        (
            self.stream.as_mut().map(|s| s.next(rate)).unwrap_or(0),
            self.volume as u16,
        )
    }
}

pub struct MixerStream {
    tones: [Unit<ToneStream>; 2],
    wave: Unit<WaveStream>,
    noise: Unit<NoiseStream>,
    enable: bool,
}

impl MixerStream {
    fn new() -> Self {
        Self {
            tones: [Unit::new(), Unit::new()],
            wave: Unit::new(),
            noise: Unit::new(),
            enable: false,
        }
    }

    fn volume(&self, amp: u16, vol: u16) -> u16 {
        amp * vol
    }
}

impl Stream for MixerStream {
    fn max(&self) -> u16 {
        // volume max = 7 * 2 = 14
        // amplitude max = 15
        // total volume max = 14 * 15 * 4 = 840
        // * 3 to soften the sound
        840 * 3
    }

    fn next(&mut self, rate: u32) -> u16 {
        if self.enable {
            let mut vol = 0;

            let (t, v) = self.tones[0].next(rate);
            vol += self.volume(t, v);
            let (t, v) = self.tones[1].next(rate);
            vol += self.volume(t, v);
            let (t, v) = self.wave.next(rate);
            vol += self.volume(t, v);
            let (t, v) = self.noise.next(rate);
            vol += self.volume(t, v) / 2; // Soften the noise

            assert!(vol <= 840, "vol = {}", vol);

            vol
        } else {
            0
        }
    }

    fn on(&self) -> bool {
        self.enable
    }
}
