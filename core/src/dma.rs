use crate::device::IoHandler;
use crate::ic::Irq;
use crate::mmu::{MemRead, MemWrite, Mmu};
use crate::sound::MixerStream;
use log::*;

#[derive(Default)]
pub struct Dma {
    pub on: bool,
    pub src: u8,
}

impl Dma {
    pub fn step(&mut self, mmu: &mut Mmu) {
        if self.on {
            assert!(self.src <= 0x80 || self.src >= 0x9f);
            debug!("Perform DMA transfer: {:02x}", self.src);

            let src = (self.src as u16) << 8;
            for i in 0..0xa0 {
                let get = mmu.get8(src + i);
                mmu.set8(0xfe00 + i, get);
            }

            self.on = false;
        }
    }
}

impl IoHandler for Dma {
    fn on_write(&mut self, addr: u16, value: u8, _: &mut MixerStream, _: &mut Irq) -> MemWrite {
        assert_eq!(addr, 0xff46);
        debug!("Start DMA transfer: {:02x}", self.src);
        self.on = true;
        self.src = value;
        MemWrite::Block
    }

    fn on_read(&mut self, _addr: u16, _: &MixerStream, _: &Irq) -> MemRead {
        MemRead::Replace(0)
    }
}
