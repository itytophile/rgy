use crate::device::IoHandler;
use crate::hardware::Key;
use crate::ic::Irq;
use crate::mmu::{MemRead, MemWrite};
use crate::sound::MixerStream;
use crate::Hardware;
use log::*;

pub struct Joypad {
    select: u8,
    pressed: u8,
}

impl Default for Joypad {
    fn default() -> Self {
        Self {
            select: 0xff,
            pressed: 0x0f,
        }
    }
}

impl Joypad {
    pub fn poll(&mut self, irq: &mut Irq, hw: &mut impl Hardware) {
        let pressed = self.check(hw);

        for i in 0..4 {
            let bit = 1 << i;
            if self.pressed & bit != 0 && pressed & bit == 0 {
                irq.joypad(true);
                break;
            }
        }

        self.pressed = pressed;
    }

    fn check(&self, hw: &mut impl Hardware) -> u8 {
        let mut value = 0;

        if self.select & 0x10 == 0 {
            value |= if hw.joypad_pressed(Key::Right) {
                0x00
            } else {
                0x01
            };
            value |= if hw.joypad_pressed(Key::Left) {
                0x00
            } else {
                0x02
            };
            value |= if hw.joypad_pressed(Key::Up) {
                0x00
            } else {
                0x04
            };
            value |= if hw.joypad_pressed(Key::Down) {
                0x00
            } else {
                0x08
            };
        } else if self.select & 0x20 == 0 {
            value |= if hw.joypad_pressed(Key::A) {
                0x00
            } else {
                0x01
            };
            value |= if hw.joypad_pressed(Key::B) {
                0x00
            } else {
                0x02
            };
            value |= if hw.joypad_pressed(Key::Select) {
                0x00
            } else {
                0x04
            };
            value |= if hw.joypad_pressed(Key::Start) {
                0x0
            } else {
                0x08
            };
        } else {
            value = 0x0f;
        }

        value
    }
}

impl IoHandler for Joypad {
    fn on_read(&mut self, addr: u16, _: &MixerStream, _: &Irq, hw: &mut impl Hardware) -> MemRead {
        if addr == 0xff00 {
            debug!("Joypad read: dir: {:02x}", self.select);

            MemRead::Replace(self.check(hw))
        } else {
            MemRead::PassThrough
        }
    }

    fn on_write(
        &mut self,
        addr: u16,
        value: u8,
        _: &mut MixerStream,
        _: &mut Irq,
        _: &mut impl Hardware,
    ) -> MemWrite {
        if addr == 0xff00 {
            self.select = value & 0xf0;
        }
        MemWrite::PassThrough
    }
}
