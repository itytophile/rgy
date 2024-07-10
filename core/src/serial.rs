use crate::{ic::Irq, Hardware};
use log::*;

pub struct Serial {
    data: u8,
    recv: u8,
    ctrl: u8,
    clock: usize,
}

impl Serial {
    pub fn new() -> Self {
        Self {
            data: 0,
            recv: 0,
            ctrl: 0,
            clock: 0,
        }
    }

    pub fn step(&mut self, time: usize, irq: &mut Irq, hw: &mut impl Hardware) {
        if self.ctrl & 0x80 == 0 {
            // No transfer
            return;
        }

        if self.ctrl & 0x01 != 0 {
            if self.clock < time {
                debug!("Serial transfer completed");
                self.data = self.recv;

                // End of transfer
                self.ctrl &= !0x80;
                irq.serial(true);
            } else {
                self.clock -= time;
            }
        } else if let Some(data) = hw.recv_byte() {
            hw.send_byte(self.data);
            self.data = data;

            // End of transfer
            self.ctrl &= !0x80;
            irq.serial(true);
        }
    }

    pub(crate) fn get_data(&self) -> u8 {
        self.data
    }

    pub(crate) fn get_ctrl(&self) -> u8 {
        self.ctrl
    }

    pub(crate) fn set_data(&mut self, value: u8) {
        self.data = value;
    }

    pub(crate) fn set_ctrl(&mut self, value: u8, hw: &mut impl Hardware) {
        self.ctrl = value;

        if self.ctrl & 0x80 != 0 {
            if self.ctrl & 0x01 != 0 {
                debug!("Serial transfer (Internal): {:02x}", self.data);

                // Internal clock is 8192 Hz = 512 cpu clocks
                self.clock = 512 * 8;

                // Do transfer one byte at once
                hw.send_byte(self.data);
                self.recv = hw.recv_byte().unwrap_or(0xff);
            } else {
                debug!("Serial transfer (External): {:02x}", self.data);
            }
        }
    }
}
