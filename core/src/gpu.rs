use crate::device::IoHandler;
use crate::ic::Irq;
use crate::mmu::{MemRead, MemWrite};
use crate::sound::MixerStream;
use crate::Hardware;
use log::*;

#[derive(Debug, Clone)]
#[allow(clippy::upper_case_acronyms)]
pub enum Mode {
    OAM,
    VRAM,
    HBlank,
    VBlank,
    None,
}

impl From<Mode> for u8 {
    fn from(v: Mode) -> u8 {
        match v {
            Mode::HBlank => 0,
            Mode::VBlank => 1,
            Mode::OAM => 2,
            Mode::VRAM => 3,
            Mode::None => 0,
        }
    }
}

impl From<u8> for Mode {
    fn from(v: u8) -> Mode {
        match v {
            0 => Mode::HBlank,
            1 => Mode::VBlank,
            2 => Mode::OAM,
            3 => Mode::VRAM,
            _ => Mode::None,
        }
    }
}

pub struct Gpu {
    pub clocks: usize,

    pub lyc_interrupt: bool,
    pub oam_interrupt: bool,
    pub vblank_interrupt: bool,
    pub hblank_interrupt: bool,
    pub mode: Mode,

    pub ly: u8,
    pub lyc: u8,
    pub scy: u8,
    pub scx: u8,

    pub wx: u8,
    pub wy: u8,

    pub enable: bool,
    pub winmap: u16,
    pub winenable: bool,
    pub tiles: u16,
    pub bgmap: u16,
    pub spsize: u16,
    pub spenable: bool,
    pub bgenable: bool,

    pub bg_palette: [Color; 4],
    pub obj_palette0: [Color; 4],
    pub obj_palette1: [Color; 4],
    pub bg_color_palette: ColorPalette,
    pub obj_color_palette: ColorPalette,
    pub vram: [[u8; 0x2000]; 2],
    pub vram_select: usize,

    pub hdma: Hdma,
}

fn to_palette(p: u8) -> [Color; 4] {
    [
        (p & 0x3).into(),
        ((p >> 2) & 0x3).into(),
        ((p >> 4) & 0x3).into(),
        ((p >> 6) & 0x3).into(),
    ]
}

fn from_palette(p: &[Color; 4]) -> u8 {
    assert_eq!(p.len(), 4);

    u8::from(p[0]) | u8::from(p[1]) << 2 | u8::from(p[2]) << 4 | u8::from(p[3]) << 6
}

#[allow(unused)]
struct SpriteAttribute<'a> {
    ypos: u16,
    xpos: u16,
    ti: u16,
    attr: MapAttribute<'a>,
}

pub struct MapAttribute<'a> {
    pub palette: &'a [Color],
    pub vram_bank: usize,
    pub xflip: bool,
    pub yflip: bool,
    pub priority: bool,
}

pub struct ColorPalette {
    cols: [[Color; 4]; 8],
    index: usize,
    auto_inc: bool,
}

impl ColorPalette {
    fn new() -> Self {
        Self {
            cols: [[Color::rgb(); 4]; 8],
            index: 0,
            auto_inc: false,
        }
    }

    fn select(&mut self, value: u8) {
        self.auto_inc = value & 0x80 != 0;
        self.index = value as usize & 0x3f;
    }

    fn read(&self) -> u8 {
        let idx = self.index / 8;
        let off = self.index % 8;

        if off % 2 == 0 {
            self.cols[idx][off / 2].get_low()
        } else {
            self.cols[idx][off / 2].get_high()
        }
    }

    fn write(&mut self, value: u8) {
        let idx = self.index / 8;
        let off = self.index % 8;

        if off % 2 == 0 {
            self.cols[idx][off / 2].set_low(value)
        } else {
            self.cols[idx][off / 2].set_high(value)
        }

        if self.auto_inc {
            self.index = (self.index + 1) % 0x40;
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum Color {
    White,
    LightGray,
    DarkGray,
    Black,
    Rgb(u8, u8, u8),
}

impl Color {
    fn rgb() -> Self {
        Color::Rgb(0, 0, 0)
    }

    fn set_low(&mut self, low: u8) {
        match *self {
            Color::Rgb(_, g, b) => {
                let nr = low & 0x1f;
                let ng = g & !0x7 | low >> 5;
                *self = Color::Rgb(nr, ng, b);
            }
            _ => unreachable!(),
        }
    }

    fn set_high(&mut self, high: u8) {
        match *self {
            Color::Rgb(r, g, _) => {
                let ng = g & !0x18 | (high & 0x3) << 3;
                let nb = (high >> 2) & 0x1f;
                *self = Color::Rgb(r, ng, nb);
            }
            _ => unreachable!(),
        }
    }

    fn get_low(&self) -> u8 {
        match *self {
            Color::Rgb(r, g, _) => (r & 0x1f) | (g & 0x7) << 5,
            _ => unreachable!(),
        }
    }

    fn get_high(&self) -> u8 {
        match *self {
            Color::Rgb(_, g, b) => ((g >> 3) & 0x3) | (b & 0x1f) << 2,
            _ => unreachable!(),
        }
    }
}

fn color_adjust(v: u8) -> u32 {
    let v = v as u32;

    if v >= 0x10 {
        0xff - (0x1f - v)
    } else {
        v
    }
}

impl From<Color> for u32 {
    fn from(c: Color) -> u32 {
        match c {
            Color::White => 0xdddddd,
            Color::LightGray => 0xaaaaaa,
            Color::DarkGray => 0x888888,
            Color::Black => 0x555555,
            Color::Rgb(r, g, b) => {
                let mut c = 0;
                c |= color_adjust(r) << 16;
                c |= color_adjust(g) << 8;
                c |= color_adjust(b);
                c
            }
        }
    }
}

impl From<Color> for u8 {
    fn from(c: Color) -> u8 {
        match c {
            Color::White => 0,
            Color::LightGray => 1,
            Color::DarkGray => 2,
            Color::Black => 3,
            _ => unreachable!(),
        }
    }
}

impl From<u8> for Color {
    fn from(v: u8) -> Color {
        match v {
            0 => Color::White,
            1 => Color::LightGray,
            2 => Color::DarkGray,
            3 => Color::Black,
            _ => unreachable!(),
        }
    }
}

pub struct Hdma {
    on: bool,
    src_low: u8,
    src_high: u8,
    dst_low: u8,
    dst_high: u8,
    src_wip: u16,
    dst_wip: u16,
    len: u8,
    hblank: bool,
}

impl Hdma {
    fn new() -> Self {
        Self {
            on: false,
            src_low: 0,
            src_high: 0,
            dst_low: 0,
            dst_high: 0,
            src_wip: 0,
            dst_wip: 0,
            len: 0,
            hblank: false,
        }
    }

    fn start(&mut self, value: u8) {
        if self.on && self.hblank && value & 0x80 == 0 {
            self.on = false;
            self.hblank = false;

            debug!("Cancel HDMA transfer");
        } else {
            self.hblank = value & 0x80 != 0;
            self.len = value & 0x7f;
            self.src_wip = ((self.src_high as u16) << 8 | self.src_low as u16) & !0x000f;
            self.dst_wip = ((self.dst_high as u16) << 8 | self.dst_low as u16) & !0xe00f | 0x8000;
            self.on = true;

            info!(
                "Start HDMA transfer: {:04x} -> {:04x} ({}) {}",
                self.src_wip, self.dst_wip, self.len, self.hblank
            );
        }
    }

    pub fn run(&mut self) -> Option<(u16, u16, u16)> {
        if self.on {
            let size = if self.hblank {
                0x10
            } else {
                (self.len as u16 + 1) * 0x10
            };

            info!(
                "HDMA transfer: {:04x} -> {:04x} ({})",
                self.src_wip, self.dst_wip, size
            );

            let range = (self.dst_wip, self.src_wip, size);

            self.src_wip += size;
            self.dst_wip += size;
            let (rem, of) = self.len.overflowing_sub(1);

            self.len = if self.hblank { rem } else { 0xff };
            self.on = if self.hblank { !of } else { false };

            Some(range)
        } else {
            None
        }
    }
}

impl Default for Gpu {
    fn default() -> Self {
        Self {
            clocks: 0,
            lyc_interrupt: false,
            oam_interrupt: false,
            vblank_interrupt: false,
            hblank_interrupt: false,
            mode: Mode::None,
            ly: 0,
            lyc: 0,
            scy: 0,
            scx: 0,
            wx: 0,
            wy: 0,
            enable: false,
            winmap: 0x9800,
            winenable: false,
            tiles: 0x8800,
            bgmap: 0x9800,
            spsize: 8,
            spenable: false,
            bgenable: false,
            bg_palette: [
                Color::White,
                Color::LightGray,
                Color::DarkGray,
                Color::Black,
            ],
            obj_palette0: [
                Color::White,
                Color::LightGray,
                Color::DarkGray,
                Color::Black,
            ],
            obj_palette1: [
                Color::White,
                Color::LightGray,
                Color::DarkGray,
                Color::Black,
            ],
            bg_color_palette: ColorPalette::new(),
            obj_color_palette: ColorPalette::new(),
            vram: [[0; 0x2000]; 2],
            vram_select: 0,
            hdma: Hdma::new(),
        }
    }
}

impl Gpu {
    fn on_write_ctrl(&mut self, value: u8, irq: &mut Irq) {
        let old_enable = self.enable;

        self.enable = value & 0x80 != 0;
        self.winmap = if value & 0x40 != 0 { 0x9c00 } else { 0x9800 };
        self.winenable = value & 0x20 != 0;
        self.tiles = if value & 0x10 != 0 { 0x8000 } else { 0x8800 };
        self.bgmap = if value & 0x08 != 0 { 0x9c00 } else { 0x9800 };
        self.spsize = if value & 0x04 != 0 { 16 } else { 8 };
        self.spenable = value & 0x02 != 0;
        self.bgenable = value & 0x01 != 0;

        if !old_enable && self.enable {
            info!("LCD enabled");
            self.clocks = 0;
            self.mode = Mode::HBlank;
            irq.vblank(false);
        } else if old_enable && !self.enable {
            info!("LCD disabled");
            self.mode = Mode::None;
            irq.vblank(false);
        }

        debug!("Write ctrl: {:02x}", value);
        debug!("Window base: {:04x}", self.winmap);
        debug!("Window enable: {}", self.winenable);
        debug!("Bg/window base: {:04x}", self.tiles);
        debug!("Background base: {:04x}", self.bgmap);
        debug!("Sprite size: 8x{}", self.spsize);
        debug!("Sprite enable: {}", self.spenable);
        debug!("Background enable: {}", self.bgenable);
    }

    fn on_write_status(&mut self, value: u8) {
        self.lyc_interrupt = value & 0x40 != 0;
        self.oam_interrupt = value & 0x20 != 0;
        self.vblank_interrupt = value & 0x10 != 0;
        self.hblank_interrupt = value & 0x08 != 0;

        debug!("LYC interrupt: {}", self.lyc_interrupt);
        debug!("OAM interrupt: {}", self.oam_interrupt);
        debug!("VBlank interrupt: {}", self.vblank_interrupt);
        debug!("HBlank interrupt: {}", self.hblank_interrupt);
    }

    fn on_read_ctrl(&mut self) -> u8 {
        let mut v = 0;
        v |= if self.enable { 0x80 } else { 0x00 };
        v |= if self.winmap == 0x9c00 { 0x40 } else { 0x00 };
        v |= if self.winenable { 0x20 } else { 0x00 };
        v |= if self.tiles == 0x8000 { 0x10 } else { 0x00 };
        v |= if self.bgmap == 0x9c00 { 0x08 } else { 0x00 };
        v |= if self.spsize == 16 { 0x04 } else { 0x00 };
        v |= if self.spenable { 0x02 } else { 0x00 };
        v |= if self.bgenable { 0x01 } else { 0x00 };
        v
    }

    fn on_read_status(&mut self) -> u8 {
        let mut v = 0;
        v |= if self.lyc_interrupt { 0x40 } else { 0x00 };
        v |= if self.oam_interrupt { 0x20 } else { 0x00 };
        v |= if self.vblank_interrupt { 0x10 } else { 0x00 };
        v |= if self.hblank_interrupt { 0x08 } else { 0x00 };
        v |= if self.ly == self.lyc { 0x04 } else { 0x00 };
        v |= {
            let p: u8 = self.mode.clone().into();
            p
        };
        trace!("Read Status: {:02x}", v);
        v
    }

    fn read_vram(&self, addr: u16, bank: usize) -> u8 {
        let off = addr as usize - 0x8000;
        self.vram[bank][off]
    }

    pub fn write_vram(&mut self, addr: u16, value: u8, bank: usize) {
        let off = addr as usize - 0x8000;
        self.vram[bank][off] = value;
    }

    pub fn get_tile_base(&self, mapbase: u16, tx: u16, ty: u16) -> u16 {
        let ti = tx + ty * 32;
        let num = self.read_vram(mapbase + ti, 0);

        if self.tiles == 0x8000 {
            self.tiles + num as u16 * 16
        } else {
            self.tiles + (0x800 + num as i8 as i16 * 16) as u16
        }
    }

    pub fn get_tile_attr(&self, mapbase: u16, tx: u16, ty: u16) -> MapAttribute {
        if cfg!(feature = "color") {
            let ti = tx + ty * 32;
            let attr = self.read_vram(mapbase + ti, 1) as usize;

            MapAttribute {
                palette: &self.bg_color_palette.cols[attr & 0x7][..],
                vram_bank: (attr >> 3) & 1,
                xflip: attr & 0x20 != 0,
                yflip: attr & 0x40 != 0,
                priority: attr & 0x80 != 0,
            }
        } else {
            MapAttribute {
                palette: &self.bg_palette,
                vram_bank: 0,
                xflip: false,
                yflip: false,
                priority: false,
            }
        }
    }

    pub fn get_sp_attr(&self, attr: u8) -> MapAttribute {
        if cfg!(feature = "color") {
            let attr = attr as usize;

            MapAttribute {
                palette: &self.obj_color_palette.cols[attr & 0x7][..],
                vram_bank: (attr >> 3) & 1,
                xflip: attr & 0x20 != 0,
                yflip: attr & 0x40 != 0,
                priority: attr & 0x80 != 0,
            }
        } else {
            let palette = if attr & 0x10 != 0 {
                &self.obj_palette1
            } else {
                &self.obj_palette0
            };

            MapAttribute {
                palette,
                vram_bank: 0,
                xflip: attr & 0x20 != 0,
                yflip: attr & 0x40 != 0,
                priority: attr & 0x80 != 0,
            }
        }
    }

    pub fn get_tile_byte(&self, tilebase: u16, txoff: u16, tyoff: u16, bank: usize) -> usize {
        let l = self.read_vram(tilebase + tyoff * 2, bank);
        let h = self.read_vram(tilebase + tyoff * 2 + 1, bank);

        let l = (l >> (7 - txoff)) & 1;
        let h = ((h >> (7 - txoff)) & 1) << 1;

        (h | l) as usize
    }
}

impl IoHandler for Gpu {
    fn on_read(&mut self, addr: u16, _: &MixerStream, _: &Irq, _: &mut impl Hardware) -> MemRead {
        if (0x8000..=0x9fff).contains(&addr) {
            MemRead(self.read_vram(addr, self.vram_select))
        } else if addr == 0xff40 {
            MemRead(self.on_read_ctrl())
        } else if addr == 0xff41 {
            MemRead(self.on_read_status())
        } else if addr == 0xff42 {
            MemRead(self.scy)
        } else if addr == 0xff43 {
            MemRead(self.scx)
        } else if addr == 0xff44 {
            MemRead(self.ly)
        } else if addr == 0xff45 {
            MemRead(self.lyc)
        } else if addr == 0xff46 {
            unreachable!("DMA request")
        } else if addr == 0xff47 {
            debug!("Read Bg palette");
            MemRead(from_palette(&self.bg_palette))
        } else if addr == 0xff48 {
            debug!("Read Object palette 0");
            MemRead(from_palette(&self.obj_palette0))
        } else if addr == 0xff49 {
            debug!("Read Object palette 1");
            MemRead(from_palette(&self.obj_palette1))
        } else if addr == 0xff4a {
            MemRead(self.wy)
        } else if addr == 0xff4b {
            MemRead(self.wx)
        } else if addr == 0xff4f {
            MemRead(self.vram_select as u8 & 0xfe)
        } else if addr == 0xff51 {
            MemRead(self.hdma.src_high)
        } else if addr == 0xff52 {
            MemRead(self.hdma.src_low)
        } else if addr == 0xff53 {
            MemRead(self.hdma.dst_high)
        } else if addr == 0xff54 {
            MemRead(self.hdma.dst_low)
        } else if addr == 0xff55 {
            let mut v = 0;
            v |= self.hdma.len & 0x7f;
            v |= if self.hdma.on { 0x00 } else { 0x80 };
            MemRead(v)
        }
        // else if addr == 0xff68 {
        //     MemRead::PassThrough
        // }
        else if addr == 0xff69 {
            MemRead(self.bg_color_palette.read())
        }
        // else if addr == 0xff6a {
        //     MemRead::PassThrough
        // }
        else if addr == 0xff6b {
            MemRead(self.obj_color_palette.read())
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
        trace!("Write GPU register: {:04x} {:02x}", addr, value);
        if (0x8000..=0x9fff).contains(&addr) {
            self.write_vram(addr, value, self.vram_select);
        } else if addr == 0xff40 {
            self.on_write_ctrl(value, irq);
        } else if addr == 0xff41 {
            self.on_write_status(value);
        } else if addr == 0xff42 {
            self.scy = value;
        } else if addr == 0xff43 {
            debug!("Write SCX: {}", value);
            self.scx = value;
        } else if addr == 0xff44 {
            self.ly = 0;
        } else if addr == 0xff45 {
            self.lyc = value;
        } else if addr == 0xff46 {
            unreachable!("Request DMA: {:02x}", value);
        } else if addr == 0xff47 {
            self.bg_palette = to_palette(value);
            debug!("Bg palette updated: {:?}", self.bg_palette);
        } else if addr == 0xff48 {
            self.obj_palette0 = to_palette(value);
            debug!("Object palette 0 updated: {:?}", self.obj_palette0);
        } else if addr == 0xff49 {
            self.obj_palette1 = to_palette(value);
            debug!("Object palette 1 updated: {:?}", self.obj_palette1);
        } else if addr == 0xff4a {
            debug!("Window Y: {}", value);
            self.wy = value;
        } else if addr == 0xff4b {
            debug!("Window X: {}", value);
            self.wx = value;
        } else if addr == 0xff4f {
            self.vram_select = value as usize & 1;
        } else if addr == 0xff51 {
            self.hdma.src_high = value;
        } else if addr == 0xff52 {
            self.hdma.src_low = value;
        } else if addr == 0xff53 {
            self.hdma.dst_high = value;
        } else if addr == 0xff54 {
            self.hdma.dst_low = value;
        } else if addr == 0xff55 {
            self.hdma.start(value);
        } else if addr == 0xff68 {
            self.bg_color_palette.select(value);
        } else if addr == 0xff69 {
            self.bg_color_palette.write(value);
        } else if addr == 0xff6a {
            self.obj_color_palette.select(value);
        } else if addr == 0xff6b {
            self.obj_color_palette.write(value);
        } else {
            warn!(
                "Unsupported GPU register is written: {:04x} {:02x}",
                addr, value
            );
        }

        MemWrite::PassThrough
    }
}
