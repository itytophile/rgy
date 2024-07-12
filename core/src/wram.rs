use core::num::NonZeroU8;

use log::*;

pub struct WramCgbExtension {
    banks: [[u8; 0x1000]; 7],
    bank_index: NonZeroU8,
}

impl Default for WramCgbExtension {
    fn default() -> Self {
        Self {
            banks: [[0; 0x1000]; 7],
            bank_index: NonZeroU8::MIN,
        }
    }
}

pub trait CgbExt: Default {
    fn select_bank(&mut self, n: u8);
    fn get_bank(&self) -> NonZeroU8;
    fn get8_post0xd000(&self, bank1: &[u8; 0x1000], addr: u16) -> u8;
    fn set8_post0xd000(&mut self, bank1: &mut [u8; 0x1000], addr: u16, value: u8);
}

impl CgbExt for () {
    fn select_bank(&mut self, _: u8) {
        info!("Can't select bank: DMG mode");
    }

    fn get_bank(&self) -> NonZeroU8 {
        panic!("DMG mode")
    }

    fn get8_post0xd000(&self, bank1: &[u8; 0x1000], addr: u16) -> u8 {
        bank1[usize::from(addr)]
    }

    fn set8_post0xd000(&mut self, bank1: &mut [u8; 0x1000], addr: u16, value: u8) {
        bank1[usize::from(addr)] = value;
    }
}

impl CgbExt for WramCgbExtension {
    fn select_bank(&mut self, n: u8) {
        self.bank_index = NonZeroU8::new(n & 0x7).unwrap_or(NonZeroU8::MIN);
        info!("WRAM bank selected: {:02x}", self.bank_index)
    }

    fn get_bank(&self) -> NonZeroU8 {
        self.bank_index
    }

    fn get8_post0xd000(&self, bank1: &[u8; 0x1000], addr: u16) -> u8 {
        self.bank_index
            .get()
            // if bank_index >= 2 then we have to the CGB wram
            .checked_sub(2)
            .map(|bank| &self.banks[usize::from(bank)])
            .unwrap_or(bank1)[usize::from(addr)]
    }

    fn set8_post0xd000(&mut self, bank1: &mut [u8; 0x1000], addr: u16, value: u8) {
        self.bank_index
            .get()
            // if bank_index >= 2 then we have to the CGB wram
            .checked_sub(2)
            .map(|bank| &mut self.banks[usize::from(bank)])
            .unwrap_or(bank1)[usize::from(addr)] = value;
    }
}

/// Handles work ram access between 0xc000 - 0xdfff
pub struct Wram<Ext: CgbExt> {
    banks: [[u8; 0x1000]; 2],
    cgb_ext: Ext,
}

impl<Ext: CgbExt> Wram<Ext> {
    pub fn new() -> Self {
        Self {
            banks: [[0; 0x1000]; 2],
            cgb_ext: Ext::default(),
        }
    }

    pub fn select_bank(&mut self, n: u8) {
        self.cgb_ext.select_bank(n);
    }

    pub fn get_bank(&self) -> NonZeroU8 {
        self.cgb_ext.get_bank()
    }

    pub fn get8(&self, addr: u16) -> u8 {
        match addr {
            0xc000..=0xcfff => self.banks[0][addr as usize - 0xc000],
            0xd000..=0xdfff => self.cgb_ext.get8_post0xd000(&self.banks[1], addr - 0xd000),
            0xe000..=0xfdff => self.get8(addr - 0xe000 + 0xc000),
            _ => unreachable!("read attemp to wram addr={:04x}", addr),
        }
    }

    pub fn set8(&mut self, addr: u16, v: u8) {
        match addr {
            0xc000..=0xcfff => self.banks[0][addr as usize - 0xc000] = v,
            0xd000..=0xdfff => self
                .cgb_ext
                .set8_post0xd000(&mut self.banks[1], addr - 0xd000, v),
            0xe000..=0xfdff => self.set8(addr - 0xe000 + 0xc000, v),
            _ => unreachable!("write attemp to wram addr={:04x} v={:02x}", addr, v),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_default_bank() {
        let m = Wram::<WramCgbExtension>::new();
        assert_eq!(m.get_bank(), NonZeroU8::MIN);
    }

    #[test]
    fn test_select_bank() {
        let mut m = Wram::<WramCgbExtension>::new();

        // fill bank 0
        for i in 0..0x1000 {
            m.set8(0xc000 + i, 0xf)
        }

        // fill bank 1-7
        for b in 1..8 {
            m.select_bank(b);
            for i in 0..0x1000 {
                m.set8(0xd000 + i, b)
            }
        }

        // check each bank
        for b in 1..8 {
            m.select_bank(b);
            for i in 0..0x1000 {
                assert_eq!(m.get8(0xc000 + i), 0xf);
                assert_eq!(m.get8(0xd000 + i), b);
            }
        }
    }

    #[test]
    fn test_no_select_bank() {
        let mut m = Wram::<()>::new();

        // fill bank 0
        for i in 0..0x1000 {
            m.set8(0xc000 + i, 0xf)
        }

        // fill bank 1-7
        for b in 1..8 {
            m.select_bank(b);
            for i in 0..0x1000 {
                m.set8(0xd000 + i, b)
            }
        }

        // check each bank (no bank switch, so always 7)
        for b in 1..8 {
            m.select_bank(b);
            for i in 0..0x1000 {
                assert_eq!(m.get8(0xc000 + i), 0xf);
                assert_eq!(m.get8(0xd000 + i), 7);
            }
        }
    }

    #[test]
    fn test_mirror() {
        let mut m = Wram::<WramCgbExtension>::new();

        for v in 1..4 {
            for i in 0..0x1dff {
                m.set8(0xc000 + i, v);
            }
            for i in 0..0x1dff {
                assert_eq!(m.get8(0xe000 + i), v);
            }
        }
    }

    #[test]
    #[should_panic]
    fn test_set_too_low() {
        let mut m = Wram::<WramCgbExtension>::new();
        m.set8(0xbfff, 0);
    }

    #[test]
    #[should_panic]
    fn test_set_too_high() {
        let mut m = Wram::<WramCgbExtension>::new();
        m.set8(0xfe00, 0);
    }

    #[test]
    #[should_panic]
    fn test_get_too_low() {
        let m = Wram::<WramCgbExtension>::new();
        m.get8(0xbfff);
    }

    #[test]
    #[should_panic]
    fn test_get_too_high() {
        let m = Wram::<WramCgbExtension>::new();
        m.get8(0xfe00);
    }
}
