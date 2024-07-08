// https://gbdev.io/pandocs/Memory_Map.html#memory-map

use crate::{device::IoHandler, mmu::MemRead};

pub const START: u16 = 0xfe00;
pub const END: u16 = 0xfe9f;

pub struct Oam([u8; END as usize - START as usize + 1]);

impl Default for Oam {
    fn default() -> Self {
        Self([0; END as usize - START as usize + 1])
    }
}

impl IoHandler for Oam {
    fn on_read(
        &mut self,
        addr: u16,
        _: &crate::sound::MixerStream,
        _: &crate::ic::Irq,
        _: &mut impl crate::Hardware,
    ) -> crate::mmu::MemRead {
        MemRead(self.0[usize::from(addr.checked_sub(START).unwrap())])
    }

    fn on_write(
        &mut self,
        addr: u16,
        value: u8,
        _: &mut crate::sound::MixerStream,
        _: &mut crate::ic::Irq,
        _: &mut impl crate::Hardware,
    ) {
        self.0[usize::from(addr.checked_sub(START).unwrap())] = value;
    }
}
