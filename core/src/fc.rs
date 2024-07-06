use crate::hardware::HardwareHandle;
use crate::system::Config;
use log::*;

pub struct FreqControl<'a> {
    hw: HardwareHandle<'a>,
    last: u64,
    cycles: u64,
    sample: u64,
    delay: u64,
    delay_unit: u64,
    target_freq: u64,
}

impl<'a> FreqControl<'a> {
    pub fn new(hw: HardwareHandle<'a>, cfg: &Config) -> Self {
        Self {
            hw,
            last: 0,
            cycles: 0,
            delay: 0,
            sample: cfg.sample,
            delay_unit: cfg.delay_unit,
            target_freq: cfg.freq,
        }
    }

    pub fn reset(&mut self) {
        self.last = self.hw.get().borrow_mut().clock();
    }

    pub fn adjust(&mut self, time: usize) {
        self.cycles += time as u64;

        if self.cycles > self.sample {
            self.cycles -= self.sample;

            let now = self.hw.get().borrow_mut().clock();
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

    pub fn delay(&self) -> u64 {
        self.delay
    }
}
