use crate::device::IoHandler;
use crate::ic::Irq;
use crate::mmu::{MemRead, MemWrite};
use crate::sound::MixerStream;
use crate::Hardware;
use log::*;

#[derive(Default)]
pub struct Serial {
    data: u8,
    recv: u8,
    ctrl: u8,
    clock: usize,
}

impl Serial {
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
}

impl IoHandler for Serial {
    fn on_read(&mut self, addr: u16, _: &MixerStream, _: &Irq, _: &mut impl Hardware) -> MemRead {
        if addr == 0xff01 {
            MemRead(self.data)
        } else if addr == 0xff02 {
            MemRead(self.ctrl)
        } else {
            unreachable!("Read from serial: {:04x}", addr)
        }
    }

    fn on_write(
        &mut self,
        addr: u16,
        value: u8,
        _: &mut MixerStream,
        _: &mut Irq,
        hw: &mut impl Hardware,
    ) -> MemWrite {
        if addr == 0xff01 {
            self.data = value;
            MemWrite::Block
        } else if addr == 0xff02 {
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
            MemWrite::Block
        } else {
            unreachable!("Write to serial: {:04x} {:02x}", addr, value)
        }
    }
}
