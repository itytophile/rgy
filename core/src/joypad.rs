use crate::hardware::Key;
use crate::ic::Irq;
use crate::Hardware;
use log::*;

pub struct Joypad {
    select: u8,
    pressed: u8,
}

impl Joypad {
    pub fn new() -> Self {
        Self {
            select: 0xff,
            pressed: 0x0f,
        }
    }

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
        let mut p = |key| hw.joypad_pressed(key);

        let mut value = 0;

        if self.select & 0x10 == 0 {
            value |= if p(Key::Right) { 0x00 } else { 0x01 };
            value |= if p(Key::Left) { 0x00 } else { 0x02 };
            value |= if p(Key::Up) { 0x00 } else { 0x04 };
            value |= if p(Key::Down) { 0x00 } else { 0x08 };
        } else if self.select & 0x20 == 0 {
            value |= if p(Key::A) { 0x00 } else { 0x01 };
            value |= if p(Key::B) { 0x00 } else { 0x02 };
            value |= if p(Key::Select) { 0x00 } else { 0x04 };
            value |= if p(Key::Start) { 0x0 } else { 0x08 };
        } else {
            value = 0x0f;
        }

        value
    }

    pub(crate) fn read(&self, hw: &mut impl Hardware) -> u8 {
        debug!("Joypad read: dir: {:02x}", self.select);
        self.check(hw)
    }

    pub(crate) fn write(&mut self, value: u8) {
        debug!(
            "Joypad write: dir: select={:02x}, value={:02x}",
            self.select, value
        );
        self.select = value & 0xf0;
    }
}
