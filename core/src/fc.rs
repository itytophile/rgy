use crate::{system::Config, Hardware};
use log::*;

pub struct FreqControl {
    last: u64,
    cycles: u64,
    sample: u64,
    delay: u64,
    delay_unit: u64,
    target_freq: u64,
}

impl FreqControl {
    pub fn new(cfg: &Config) -> Self {
        Self {
            last: 0,
            cycles: 0,
            delay: 0,
            sample: cfg.sample,
            delay_unit: cfg.delay_unit,
            target_freq: cfg.freq,
        }
    }

    pub fn reset(&mut self, hw: &mut impl Hardware) {
        self.last = hw.clock();
    }

    pub fn adjust(&mut self, time: usize, hw: &mut impl Hardware) {
        self.cycles += time as u64;

        for _ in 0..self.delay {
            let _ = unsafe { core::ptr::read_volatile(&self.sample) };
        }

        if self.cycles > self.sample {
            self.cycles -= self.sample;

            let now = hw.clock();
            let (diff, of) = now.overflowing_sub(self.last);
            if of || diff == 0 {
                warn!("Overflow: {} - {}", self.last, now);
                self.last = now;
                return;
            }

            // get cycles per second
            let freq = self.sample * 1_000_000 / diff;

            debug!("Frequency: {}", freq);

            self.delay = if freq > self.target_freq {
                self.delay.saturating_add(self.delay_unit)
            } else {
                self.delay.saturating_sub(self.delay_unit)
            };

            self.last = now;
        }
    }
}
