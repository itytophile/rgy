use crate::device::IoHandler;
use crate::hardware::{HardwareHandle, Key};
use crate::ic::Irq;
use crate::mmu::{MemRead, MemWrite, Mmu};
use log::*;

pub struct Joypad<'a> {
    hw: HardwareHandle<'a>,
    irq: Irq,
    select: u8,
    pressed: u8,
}

impl<'a> Joypad<'a> {
    pub fn new(hw: HardwareHandle<'a>, irq: Irq) -> Self {
        Self {
            hw,
            irq,
            select: 0xff,
            pressed: 0x0f,
        }
    }

    pub fn poll(&mut self) {
        let pressed = self.check();

        for i in 0..4 {
            let bit = 1 << i;
            if self.pressed & bit != 0 && pressed & bit == 0 {
                self.irq.joypad(true);
                break;
            }
        }

        self.pressed = pressed;
    }

    fn check(&self) -> u8 {
        let p = |key| self.hw.get().borrow_mut().joypad_pressed(key);

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
}

impl<'a> IoHandler for Joypad<'a> {
    fn on_read(&mut self, _mmu: &Mmu, addr: u16) -> MemRead {
        if addr == 0xff00 {
            debug!("Joypad read: dir: {:02x}", self.select);

            MemRead::Replace(self.check())
        } else {
            MemRead::PassThrough
        }
    }

    fn on_write(&mut self, _mmu: &Mmu, addr: u16, value: u8) -> MemWrite {
        if addr == 0xff00 {
            self.select = value & 0xf0;
        }
        MemWrite::PassThrough
    }
}
