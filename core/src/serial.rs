use crate::ic::Irq;
use arrayvec::ArrayVec;
use log::*;

#[derive(Default)]
pub struct Serial {
    data: u8,
    recv: u8,
    ctrl: u8,
    clock: usize,
    // don't know if the gameboy can send more than one byte during the serial step + cpu execution
    sent_bytes: ArrayVec<u8, 1>,
}

impl Serial {
    pub fn step(&mut self, time: usize, irq: &mut Irq, serial_input: &mut Option<u8>) {
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
        } else if let Some(data) = serial_input.take() {
            self.sent_bytes.push(self.data);
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

    pub(crate) fn set_ctrl(&mut self, value: u8, serial_input: &mut Option<u8>) {
        self.ctrl = value;

        if self.ctrl & 0x80 != 0 {
            if self.ctrl & 0x01 != 0 {
                debug!("Serial transfer (Internal): {:02x}", self.data);

                // Internal clock is 8192 Hz = 512 cpu clocks
                self.clock = 512 * 8;

                // Do transfer one byte at once
                self.sent_bytes.push(self.data);
                self.recv = serial_input.take().unwrap_or(0xff);
            } else {
                debug!("Serial transfer (External): {:02x}", self.data);
            }
        }
    }

    pub fn clear_sent_bytes(&mut self) {
        self.sent_bytes.clear();
    }

    pub fn get_sent_bytes(&self) -> &[u8] {
        &self.sent_bytes
    }
}
