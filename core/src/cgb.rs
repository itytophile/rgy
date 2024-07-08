use crate::{device::IoHandler, ic::Irq, mmu::MemRead, sound::MixerStream, Hardware};
use log::*;

pub struct Cgb {
    double_speed: bool,
    speed_switch: bool,
    wram_select: usize,
    wram_bank: [[u8; 0x1000]; 8],
}

impl Default for Cgb {
    fn default() -> Self {
        Self {
            double_speed: false,
            speed_switch: false,
            wram_select: 1,
            wram_bank: [[0; 0x1000]; 8],
        }
    }
}

#[allow(unused)]
impl Cgb {
    pub fn try_switch_speed(&mut self) {
        if self.speed_switch {
            self.double_speed = !self.double_speed;
            self.speed_switch = false;
        }
    }

    pub fn double_speed(&self) -> bool {
        self.double_speed
    }
}

impl IoHandler for Cgb {
    fn on_read(&mut self, addr: u16, _: &MixerStream, _: &Irq, _: &mut impl Hardware) -> MemRead {
        if (0xc000..=0xcfff).contains(&addr) {
            let off = addr as usize - 0xc000;
            MemRead(self.wram_bank[0][off])
        } else if (0xd000..=0xdfff).contains(&addr) {
            let off = addr as usize - 0xd000;
            MemRead(self.wram_bank[self.wram_select][off])
        } else if addr == 0xff4d {
            let mut v = 0;
            v |= if self.double_speed { 0x80 } else { 0x00 };
            v |= if self.speed_switch { 0x01 } else { 0x00 };
            MemRead(v)
        }
        // else if addr == 0xff56 {
        //     warn!("Infrared read");
        //     MemRead::PassThrough
        // }
        else if addr == 0xff70 {
            MemRead(self.wram_select as u8)
        } else {
            unreachable!()
        }
    }

    fn on_write(
        &mut self,
        addr: u16,
        value: u8,
        _: &mut MixerStream,
        _: &mut Irq,
        _: &mut impl Hardware,
    ) {
        if (0xc000..=0xcfff).contains(&addr) {
            let off = addr as usize - 0xc000;
            self.wram_bank[0][off] = value;
        } else if (0xd000..=0xdfff).contains(&addr) {
            let off = addr as usize - 0xd000;
            self.wram_bank[self.wram_select][off] = value;
        } else if addr == 0xff4d {
            self.speed_switch = value & 0x01 != 0;
        } else if addr == 0xff56 {
            warn!("Infrared read");
        } else if addr == 0xff70 {
            self.wram_select = (value as usize & 0xf).max(1);
        } else {
            unreachable!("{:x}", addr)
        }
    }
}
