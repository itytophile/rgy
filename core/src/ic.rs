use crate::device::IoHandler;
use crate::mmu::{MemRead, MemWrite};
use crate::sound::MixerStream;
use crate::Hardware;
use log::*;

#[derive(Default)]
pub struct Irq {
    request: Ints,
}

impl Irq {
    pub fn vblank(&mut self, v: bool) {
        self.request.vblank = v;
    }

    pub fn lcd(&mut self, v: bool) {
        self.request.lcd = v;
    }

    pub fn timer(&mut self, v: bool) {
        self.request.timer = v;
    }

    pub fn serial(&mut self, v: bool) {
        self.request.serial = v;
    }

    pub fn joypad(&mut self, v: bool) {
        self.request.joypad = v;
    }
}

#[derive(Debug, Default)]
struct Ints {
    vblank: bool,
    lcd: bool,
    timer: bool,
    serial: bool,
    joypad: bool,
}

impl Ints {
    fn set(&mut self, value: u8) {
        self.vblank = value & 0x01 != 0;
        self.lcd = value & 0x02 != 0;
        self.timer = value & 0x04 != 0;
        self.serial = value & 0x08 != 0;
        self.joypad = value & 0x10 != 0;
    }

    fn get(&self) -> u8 {
        let mut v = 0;
        v |= if self.vblank { 0x01 } else { 0x00 };
        v |= if self.lcd { 0x02 } else { 0x00 };
        v |= if self.timer { 0x04 } else { 0x00 };
        v |= if self.serial { 0x08 } else { 0x00 };
        v |= if self.joypad { 0x10 } else { 0x00 };
        v
    }
}

#[derive(Default)]
pub struct Ic {
    enable: Ints,
}

impl Ic {
    pub fn peek(&self, irq: &mut Irq) -> Option<u8> {
        self.check(false, irq)
    }

    pub fn poll(&self, irq: &mut Irq) -> Option<u8> {
        self.check(true, irq)
    }

    fn check(&self, consume: bool, irq: &mut Irq) -> Option<u8> {
        if self.enable.vblank && irq.request.vblank {
            irq.request.vblank = !consume;
            Some(0x40)
        } else if self.enable.lcd && irq.request.lcd {
            irq.request.lcd = !consume;
            Some(0x48)
        } else if self.enable.timer && irq.request.timer {
            irq.request.timer = !consume;
            Some(0x50)
        } else if self.enable.serial && irq.request.serial {
            irq.request.serial = !consume;
            Some(0x58)
        } else if self.enable.joypad && irq.request.joypad {
            irq.request.joypad = !consume;
            Some(0x60)
        } else {
            None
        }
    }
}

impl IoHandler for Ic {
    fn on_read(&mut self, addr: u16, _: &MixerStream, irq: &Irq, _: &mut impl Hardware) -> MemRead {
        if addr == 0xffff {
            let v = self.enable.get();
            info!("Read interrupt enable: {:02x}", v);
            MemRead(v)
        } else if addr == 0xff0f {
            let v = irq.request.get();
            info!("Read interrupt: {:02x}", v);
            MemRead(v)
        } else {
            unreachable!()
        }
    }

    fn on_write(
        &mut self,
        addr: u16,
        value: u8,
        _: &mut MixerStream,
        irq: &mut Irq,
        _: &mut impl Hardware,
    ) -> MemWrite {
        if addr == 0xffff {
            info!("Write interrupt enable: {:02x}", value);
            self.enable.set(value);
            MemWrite::Block
        } else if addr == 0xff0f {
            info!("Write interrupt: {:02x}", value);
            irq.request.set(value);
            MemWrite::Block
        } else {
            info!("Writing to IC register: {:04x}", addr);
            MemWrite::PassThrough
        }
    }
}
