use crate::device::IoHandler;
use crate::ic::Irq;
use crate::mmu::{MemRead, MemWrite};
use crate::sound::MixerStream;
use crate::Hardware;
use log::*;

#[derive(Default)]
pub struct Dma {
    pub on: bool,
    pub src: u8,
}

impl IoHandler for Dma {
    fn on_write(
        &mut self,
        addr: u16,
        value: u8,
        _: &mut MixerStream,
        _: &mut Irq,
        _: &mut impl Hardware,
    ) -> MemWrite {
        assert_eq!(addr, 0xff46);
        debug!("Start DMA transfer: {:02x}", self.src);
        self.on = true;
        self.src = value;
        MemWrite::Block
    }

    fn on_read(&mut self, _addr: u16, _: &MixerStream, _: &Irq, _: &mut impl Hardware) -> MemRead {
        MemRead::Replace(0)
    }
}
