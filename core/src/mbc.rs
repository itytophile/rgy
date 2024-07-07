use crate::device::IoHandler;
use crate::hardware::HardwareHandle;
use crate::mmu::{MemRead, MemWrite};
use crate::sound::MixerStream;
use log::*;

const BOOT_ROM: &[u8] = {
    #[cfg(feature = "color")]
    {
        include_bytes!("cgb.bin")
    }
    #[cfg(not(feature = "color"))]
    {
        include_bytes!("dmg.bin")
    }
};

struct MbcNone<'a> {
    rom: &'a [u8],
}

impl<'a> MbcNone<'a> {
    fn new(rom: &'a [u8]) -> Self {
        Self { rom }
    }

    fn on_read(&mut self, addr: u16) -> MemRead {
        if addr <= 0x7fff {
            MemRead::Replace(self.rom[addr as usize])
        } else {
            MemRead::PassThrough
        }
    }

    fn on_write(&mut self, addr: u16, value: u8) -> MemWrite {
        if addr <= 0x7fff {
            MemWrite::Block
        } else if (0xa000..=0xbfff).contains(&addr) {
            MemWrite::PassThrough
        } else {
            unreachable!("Write to ROM: {:02x} {:02x}", addr, value);
        }
    }
}

struct Mbc1<'a> {
    hw: HardwareHandle<'a>,
    rom: &'a [u8],
    ram: [u8; 0x8000],
    rom_bank: usize,
    ram_bank: usize,
    ram_enable: bool,
    ram_select: bool,
}

impl<'a> Mbc1<'a> {
    fn new(hw: HardwareHandle<'a>, rom: &'a [u8]) -> Self {
        Self {
            hw,
            rom,
            ram: [0; 0x8000],
            rom_bank: 0,
            ram_bank: 0,
            ram_enable: false,
            ram_select: false,
        }
    }

    fn on_read(&mut self, addr: u16) -> MemRead {
        if addr <= 0x3fff {
            MemRead::Replace(self.rom[addr as usize])
        } else if (0x4000..=0x7fff).contains(&addr) {
            let rom_bank = self.rom_bank.max(1);

            // ROM bank 0x20, 0x40, 0x60 are somehow not available
            let rom_bank = if rom_bank == 0x20 || rom_bank == 0x40 || rom_bank == 0x60 {
                warn!("Odd ROM bank selection: {:02x}", rom_bank);
                rom_bank + 1
            } else {
                rom_bank
            };

            let base = rom_bank * 0x4000;
            let offset = addr as usize - 0x4000;
            let addr = (base + offset) & (self.rom.len() - 1);
            MemRead::Replace(self.rom[addr])
        } else if (0xa000..=0xbfff).contains(&addr) {
            if self.ram_enable {
                let base = self.ram_bank * 0x2000;
                let offset = addr as usize - 0xa000;
                let addr = (base + offset) & (self.rom.len() - 1);
                MemRead::Replace(self.ram[addr])
            } else {
                warn!("Read from disabled external RAM: {:04x}", addr);
                MemRead::Replace(0)
            }
        } else {
            MemRead::PassThrough
        }
    }

    fn on_write(&mut self, addr: u16, value: u8) -> MemWrite {
        if addr <= 0x1fff {
            if value & 0xf == 0x0a {
                info!("External RAM enabled");
                self.ram_enable = true;
            } else {
                info!("External RAM disabled");
                self.ram_enable = false;
                self.hw.get().borrow_mut().save_ram(&self.ram);
            }
            MemWrite::Block
        } else if (0x2000..=0x3fff).contains(&addr) {
            self.rom_bank = (self.rom_bank & !0x1f) | (value as usize & 0x1f);
            debug!("Switch ROM bank to {:02x}", self.rom_bank);
            MemWrite::Block
        } else if (0x4000..=0x5fff).contains(&addr) {
            if self.ram_select {
                self.ram_bank = value as usize & 0x3;
            } else {
                self.rom_bank = (self.rom_bank & !0x60) | ((value as usize & 0x3) << 5);
            }
            MemWrite::Block
        } else if (0x6000..=0x7fff).contains(&addr) {
            if value == 0x00 {
                self.ram_select = false;
            } else if value == 0x01 {
                self.ram_select = true;
            } else {
                unimplemented!("Invalid ROM/RAM select mode");
            }
            MemWrite::Block
        } else if (0xa000..=0xbfff).contains(&addr) {
            if self.ram_enable {
                let base = self.ram_bank * 0x2000;
                let offset = addr as usize - 0xa000;
                self.ram[base + offset] = value;
                MemWrite::Block
            } else {
                warn!("Write to disabled external RAM: {:04x} {:02x}", addr, value);
                MemWrite::Block
            }
        } else {
            unimplemented!("write to rom {:04x} {:02x}", addr, value)
        }
    }
}

struct Mbc2<'a> {
    hw: HardwareHandle<'a>,
    rom: &'a [u8],
    ram: [u8; 0x200],
    rom_bank: usize,
    ram_enable: bool,
}

impl<'a> Mbc2<'a> {
    fn new(hw: HardwareHandle<'a>, rom: &'a [u8]) -> Self {
        Self {
            hw,
            rom,
            ram: [0; 0x200],
            rom_bank: 1,
            ram_enable: false,
        }
    }

    fn on_read(&mut self, addr: u16) -> MemRead {
        if addr <= 0x3fff {
            MemRead::Replace(self.rom[addr as usize])
        } else if (0x4000..=0x7fff).contains(&addr) {
            let base = self.rom_bank.max(1) * 0x4000;
            let offset = addr as usize - 0x4000;
            MemRead::Replace(self.rom[base + offset])
        } else if (0xa000..=0xa1ff).contains(&addr) {
            if self.ram_enable {
                MemRead::Replace(self.ram[addr as usize - 0xa000] & 0xf)
            } else {
                warn!("Read from disabled cart RAM: {:04x}", addr);
                MemRead::Replace(0)
            }
        } else {
            MemRead::PassThrough
        }
    }

    fn on_write(&mut self, addr: u16, value: u8) -> MemWrite {
        if addr <= 0x1fff {
            if addr & 0x100 == 0 {
                self.ram_enable = (value & 0x0f) == 0x0a;
                info!(
                    "Cart RAM {} {:02x}",
                    if self.ram_enable {
                        "enabled"
                    } else {
                        "disabled"
                    },
                    value
                );
                if !self.ram_enable {
                    self.hw.get().borrow_mut().save_ram(&self.ram);
                }
            }
            MemWrite::Block
        } else if (0x2000..=0x3fff).contains(&addr) {
            if addr & 0x100 != 0 {
                self.rom_bank = (value as usize & 0xf).max(1);
                debug!("Switch ROM bank to {:02x}", self.rom_bank);
            }
            MemWrite::Block
        } else if (0x4000..=0x7fff).contains(&addr) {
            warn!("Writing to read-only range: {:04x} {:02x}", addr, value);
            MemWrite::Block
        } else if (0xa000..=0xa1ff).contains(&addr) {
            if self.ram_enable {
                self.ram[addr as usize - 0xa000] = value & 0xf;
                MemWrite::Block
            } else {
                warn!("Write to disabled cart RAM: {:04x} {:02x}", addr, value);
                MemWrite::Block
            }
        } else {
            warn!("write to rom {:04x} {:02x}", addr, value);
            MemWrite::PassThrough
        }
    }
}

struct Mbc3<'a> {
    hw: HardwareHandle<'a>,
    rom: &'a [u8],
    ram: [u8; 0x8000],
    rom_bank: usize,
    enable: bool,
    select: u8,
    rtc_secs: u8,
    rtc_mins: u8,
    rtc_hours: u8,
    rtc_day_low: u8,
    rtc_day_high: u8,
    epoch: u64,
    prelatch: bool,
}

impl<'a> Drop for Mbc3<'a> {
    fn drop(&mut self) {
        self.save();
    }
}

impl<'a> Mbc3<'a> {
    fn new(hw: HardwareHandle<'a>, rom: &'a [u8]) -> Self {
        let mut s = Self {
            hw,
            rom,
            ram: [0; 0x8000],
            rom_bank: 0,
            enable: false,
            select: 0,
            rtc_secs: 0,
            rtc_mins: 0,
            rtc_hours: 0,
            rtc_day_low: 0,
            rtc_day_high: 0,
            epoch: 0,
            prelatch: false,
        };
        s.update_epoch();
        s
    }

    fn save(&mut self) {
        self.hw.get().borrow_mut().save_ram(&self.ram);
    }

    fn epoch(&self) -> u64 {
        self.hw.get().borrow_mut().clock() / 1_000_000
    }

    fn on_read(&mut self, addr: u16) -> MemRead {
        if addr <= 0x3fff {
            MemRead::Replace(self.rom[addr as usize])
        } else if (0x4000..=0x7fff).contains(&addr) {
            let rom_bank = self.rom_bank.max(1);
            let base = rom_bank * 0x4000;
            let offset = addr as usize - 0x4000;
            MemRead::Replace(self.rom[base + offset])
        } else if (0xa000..=0xbfff).contains(&addr) {
            match self.select {
                x if x == 0x00 || x == 0x01 || x == 0x02 || x == 0x03 => {
                    let base = x as usize * 0x2000;
                    let offset = addr as usize - 0xa000;
                    MemRead::Replace(self.ram[base + offset])
                }
                0x08 => MemRead::Replace(self.rtc_secs),
                0x09 => MemRead::Replace(self.rtc_mins),
                0x0a => MemRead::Replace(self.rtc_hours),
                0x0b => MemRead::Replace(self.rtc_day_low),
                0x0c => MemRead::Replace(self.rtc_day_high),
                s => unimplemented!("Unknown selector: {:02x}", s),
            }
        } else {
            unreachable!("Invalid read from ROM: {:02x}", addr);
        }
    }

    fn on_write(&mut self, addr: u16, value: u8) -> MemWrite {
        if addr <= 0x1fff {
            if value == 0x00 {
                info!("External RAM/RTC disabled");
                self.enable = false;
            } else if value == 0x0a {
                info!("External RAM/RTC enabled");
                self.enable = true;
            }
            MemWrite::Block
        } else if (0x2000..=0x3fff).contains(&addr) {
            self.rom_bank = value as usize & 0x7f;
            trace!("Switch ROM bank to {}", self.rom_bank);
            MemWrite::Block
        } else if (0x4000..=0x5fff).contains(&addr) {
            self.select = value;
            self.save();
            debug!("Select RAM bank/RTC: {:02x}", self.select);
            MemWrite::Block
        } else if (0x6000..=0x7fff).contains(&addr) {
            if self.prelatch {
                if value == 0x01 {
                    self.latch();
                }
                self.prelatch = false;
            } else if value == 0x00 {
                self.prelatch = true;
            }
            MemWrite::Block
        } else if (0xa000..=0xbfff).contains(&addr) {
            match self.select {
                x if x == 0x00 || x == 0x01 || x == 0x02 || x == 0x03 => {
                    let base = x as usize * 0x2000;
                    let offset = addr as usize - 0xa000;
                    self.ram[base + offset] = value;
                    MemWrite::Block
                }
                0x08 => {
                    self.rtc_secs = value;
                    self.update_epoch();
                    MemWrite::Block
                }
                0x09 => {
                    self.rtc_mins = value;
                    self.update_epoch();
                    MemWrite::Block
                }
                0x0a => {
                    self.rtc_hours = value;
                    self.update_epoch();
                    MemWrite::Block
                }
                0x0b => {
                    self.rtc_day_low = value;
                    self.update_epoch();
                    MemWrite::Block
                }
                0x0c => {
                    self.rtc_day_high = value;
                    self.update_epoch();
                    MemWrite::Block
                }
                s => unimplemented!("Unknown selector: {:02x}", s),
            }
        } else {
            unimplemented!("write to rom {:04x} {:02x}", addr, value)
        }
    }

    fn update_epoch(&mut self) {
        self.epoch = self.epoch();
    }

    fn day(&self) -> u64 {
        ((self.rtc_day_high as u64 & 1) << 8) & self.rtc_day_low as u64
    }

    fn dhms_to_secs(&self) -> u64 {
        let d = self.day();
        let s = self.rtc_secs as u64;
        let m = self.rtc_mins as u64;
        let h = self.rtc_hours as u64;
        (d * 24 + h) * 3600 + m * 60 + s
    }

    fn secs_to_dhms(&mut self, secs: u64) {
        let s = secs % 60;
        let m = (secs / 60) % 60;
        let h = (secs / 3600) % 24;
        let d = secs / (3600 * 24);
        self.rtc_secs = s as u8;
        self.rtc_mins = m as u8;
        self.rtc_hours = h as u8;
        self.rtc_day_low = d as u8;
        self.rtc_day_high = (self.rtc_day_high & !1) | ((d >> 8) & 1) as u8;
    }

    fn latch(&mut self) {
        let new_epoch = if self.rtc_day_high & 0x40 == 0 {
            self.epoch()
        } else {
            // Halt
            self.epoch
        };
        let elapsed = new_epoch - self.epoch;

        let last_day = self.day();
        let last_secs = self.dhms_to_secs();
        self.secs_to_dhms(last_secs + elapsed);
        let new_day = self.day();

        // Overflow
        if new_day < last_day {
            self.rtc_day_high |= 0x80;
        }

        debug!(
            "Latching RTC: {:04}/{:02}:{:02}:{:02}",
            self.day(),
            self.rtc_hours,
            self.rtc_mins,
            self.rtc_secs
        );

        self.epoch = new_epoch;
    }
}

#[cfg(feature = "mb5")]
struct Mbc5<'a> {
    hw: HardwareHandle<'a>,
    rom: &'a [u8],
    ram: [u8; 0x20000],
    rom_bank: usize,
    ram_bank: usize,
    ram_enable: bool,
}

#[cfg(feature = "mb5")]
impl<'a> Mbc5<'a> {
    fn new(hw: HardwareHandle<'a>, rom: &'a [u8]) -> Self {
        Self {
            hw,
            rom,
            ram: [0; 0x20000],
            rom_bank: 0,
            ram_bank: 0,
            ram_enable: false,
        }
    }

    fn on_read(&mut self, addr: u16) -> MemRead {
        if addr <= 0x3fff {
            MemRead::Replace(self.rom[addr as usize])
        } else if (0x4000..=0x7fff).contains(&addr) {
            let base = self.rom_bank * 0x4000;
            let offset = addr as usize - 0x4000;
            MemRead::Replace(self.rom[base + offset])
        } else if (0xa000..=0xbfff).contains(&addr) {
            if self.ram_enable {
                let base = self.ram_bank * 0x2000;
                let offset = addr as usize - 0xa000;
                MemRead::Replace(self.ram[base + offset])
            } else {
                warn!("Read from disabled external RAM: {:04x}", addr);
                MemRead::Replace(0)
            }
        } else {
            MemRead::PassThrough
        }
    }

    fn on_write(&mut self, addr: u16, value: u8) -> MemWrite {
        if addr <= 0x1fff {
            if value & 0xf == 0x0a {
                info!("External RAM enabled");
                self.ram_enable = true;
            } else {
                info!("External RAM disabled");
                self.ram_enable = false;
                self.hw.get().borrow_mut().save_ram(&self.ram);
            }
            MemWrite::Block
        } else if (0x2000..=0x2fff).contains(&addr) {
            self.rom_bank = (self.rom_bank & !0xff) | value as usize;
            debug!("Switch ROM bank to {:02x}", self.rom_bank);
            MemWrite::Block
        } else if (0x3000..=0x3fff).contains(&addr) {
            self.rom_bank = (self.rom_bank & !0x100) | (value as usize & 1) << 8;
            debug!("Switch ROM bank to {:02x}", self.rom_bank);
            MemWrite::Block
        } else if (0x4000..=0x5fff).contains(&addr) {
            self.ram_bank = value as usize & 0xf;
            MemWrite::Block
        } else if (0xa000..=0xbfff).contains(&addr) {
            if self.ram_enable {
                let base = self.ram_bank * 0x2000;
                let offset = addr as usize - 0xa000;
                self.ram[base + offset] = value;
                MemWrite::Block
            } else {
                warn!("Write to disabled external RAM: {:04x} {:02x}", addr, value);
                MemWrite::Block
            }
        } else {
            unimplemented!("write to rom {:04x} {:02x}", addr, value)
        }
    }
}

#[allow(unused)]
struct HuC1<'a> {
    rom: &'a [u8],
}

impl<'a> HuC1<'a> {
    fn new(rom: &'a [u8]) -> Self {
        Self { rom }
    }

    fn on_read(&mut self, _addr: u16) -> MemRead {
        unimplemented!()
    }

    fn on_write(&mut self, _addr: u16, _value: u8) -> MemWrite {
        unimplemented!()
    }
}

enum MbcType<'a> {
    None(MbcNone<'a>),
    Mbc1(Mbc1<'a>),
    Mbc2(Mbc2<'a>),
    Mbc3(Mbc3<'a>),
    #[cfg(feature = "mb5")]
    Mbc5(Mbc5<'a>),
    HuC1(HuC1<'a>),
}

impl<'a> MbcType<'a> {
    fn new(hw: HardwareHandle<'a>, code: u8, rom: &'a [u8]) -> Self {
        match code {
            0x00 => MbcType::None(MbcNone::new(rom)),
            0x01..=0x03 => MbcType::Mbc1(Mbc1::new(hw, rom)),
            0x05 | 0x06 => MbcType::Mbc2(Mbc2::new(hw, rom)),
            0x08 | 0x09 => unimplemented!("ROM+RAM: {:02x}", code),
            0x0b..=0x0d => unimplemented!("MMM01: {:02x}", code),
            0x0f..=0x13 => MbcType::Mbc3(Mbc3::new(hw, rom)),
            0x15..=0x17 => unimplemented!("Mbc4: {:02x}", code),
            0x19..=0x1e => {
                #[cfg(not(feature = "mb5"))]
                panic!("Provide the mb5 feature at build");
                #[cfg(feature = "mb5")]
                MbcType::Mbc5(Mbc5::new(hw, rom))
            }
            0xfc => unimplemented!("POCKET CAMERA"),
            0xfd => unimplemented!("BANDAI TAMAS"),
            0xfe => unimplemented!("HuC3"),
            0xff => MbcType::HuC1(HuC1::new(rom)),
            _ => unreachable!("Invalid cartridge type: {:02x}", code),
        }
    }

    fn on_read(&mut self, addr: u16) -> MemRead {
        match self {
            MbcType::None(c) => c.on_read(addr),
            MbcType::Mbc1(c) => c.on_read(addr),
            MbcType::Mbc2(c) => c.on_read(addr),
            MbcType::Mbc3(c) => c.on_read(addr),
            #[cfg(feature = "mb5")]
            MbcType::Mbc5(c) => c.on_read(addr),
            MbcType::HuC1(c) => c.on_read(addr),
        }
    }

    fn on_write(&mut self, addr: u16, value: u8) -> MemWrite {
        match self {
            MbcType::None(c) => c.on_write(addr, value),
            MbcType::Mbc1(c) => c.on_write(addr, value),
            MbcType::Mbc2(c) => c.on_write(addr, value),
            MbcType::Mbc3(c) => c.on_write(addr, value),
            #[cfg(feature = "mb5")]
            MbcType::Mbc5(c) => c.on_write(addr, value),
            MbcType::HuC1(c) => c.on_write(addr, value),
        }
    }
}

struct Cartridge<'a> {
    // cgb: bool,
    // cgb_only: bool,
    // license_old: u8,
    // sgb: bool,
    mbc: MbcType<'a>,
    // rom_size: u8,
    // ram_size: u8,
    // dstcode: u8,
    // rom_version: u8,
}

fn verify(rom: &[u8], checksum: u16) {
    let mut sum = 0u16;

    for (i, b) in rom.iter().enumerate() {
        if i == 0x14e || i == 0x14f {
            continue;
        }
        sum = sum.wrapping_add(*b as u16);
    }

    if sum == checksum {
        info!("ROM checksum verified: {:04x}", checksum);
    } else {
        warn!(
            "ROM checksum mismatch: expect: {:04x}, actual: {:04x}",
            checksum, sum
        );
    }
}

impl<'a> Cartridge<'a> {
    fn new(hw: HardwareHandle<'a>, rom: &'a [u8]) -> Self {
        let checksum = (rom[0x14e] as u16) << 8 | (rom[0x14f] as u16);

        verify(rom, checksum);

        Self {
            // cgb: rom[0x143] & 0x80 != 0,
            // cgb_only: rom[0x143] == 0xc0,
            // license_old: rom[0x14b],
            // sgb: rom[0x146] == 0x03,
            mbc: MbcType::new(hw, rom[0x147], rom),
            // rom_size: rom[0x148],
            // ram_size: rom[0x149],
            // dstcode: rom[0x14a],
            // rom_version: rom[0x14c],
        }
    }

    fn on_read(&mut self, addr: u16) -> MemRead {
        self.mbc.on_read(addr)
    }

    fn on_write(&mut self, addr: u16, value: u8) -> MemWrite {
        self.mbc.on_write(addr, value)
    }
}

pub struct Mbc<'a> {
    cartridge: Cartridge<'a>,
    use_boot_rom: bool,
}

impl<'a> Mbc<'a> {
    pub fn new(hw: HardwareHandle<'a>, rom: &'a [u8]) -> Self {
        let cartridge = Cartridge::new(hw, rom);

        Self {
            cartridge,
            use_boot_rom: true,
        }
    }

    fn in_boot_rom(&self, addr: u16) -> bool {
        if cfg!(feature = "color") {
            assert_eq!(0x900, BOOT_ROM.len());

            addr < 0x100 || (0x200..0x900).contains(&addr)
        } else {
            assert_eq!(0x100, BOOT_ROM.len());

            addr < 0x100
        }
    }
}

impl<'a> IoHandler for Mbc<'a> {
    fn on_read(&mut self, addr: u16, _: &MixerStream) -> MemRead {
        if self.use_boot_rom && self.in_boot_rom(addr) {
            MemRead::Replace(BOOT_ROM[addr as usize])
        } else {
            self.cartridge.on_read(addr)
        }
    }

    fn on_write(&mut self, addr: u16, value: u8, _: &mut MixerStream) -> MemWrite {
        if self.use_boot_rom && addr < 0x100 {
            unreachable!("Writing to boot ROM")
        } else if addr == 0xff50 {
            info!("Disable boot ROM");
            self.use_boot_rom = false;
            MemWrite::Block
        } else {
            self.cartridge.on_write(addr, value)
        }
    }
}
