use crate::dma::DmaRequest;
use crate::hardware::{VRAM_HEIGHT, VRAM_WIDTH};
use crate::ic::{Ints, Irq};
use log::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Oam,
    Vram,
    HBlank,
    VBlank,
    None,
}

impl From<Mode> for u8 {
    fn from(v: Mode) -> u8 {
        match v {
            Mode::HBlank => 0,
            Mode::VBlank => 1,
            Mode::Oam => 2,
            Mode::Vram => 3,
            Mode::None => 0,
        }
    }
}

impl From<u8> for Mode {
    fn from(v: u8) -> Mode {
        match v {
            0 => Mode::HBlank,
            1 => Mode::VBlank,
            2 => Mode::Oam,
            3 => Mode::Vram,
            _ => Mode::None,
        }
    }
}

#[derive(Clone, Copy)]
pub struct Point {
    x: u16,
    y: u16,
}

pub trait CgbExt: Default {
    type Color: Default + Copy;

    fn get_col_coli(
        &self,
        vram_bank0: &[u8; 0x2000],
        tiles: u16,
        tile: Point,
        tile_offset: Point,
        mapbase: u16,
    ) -> (Self::Color, usize);

    fn get_window_col(
        &self,
        vram_bank0: &[u8; 0x2000],
        tiles: u16,
        tile: Point,
        tile_offset: Point,
        mapbase: u16,
    ) -> Self::Color;

    fn get_sp_attr<'a>(
        &'a self,
        attr: u8,
        vram_bank0: &'a [u8; 0x2000],
    ) -> MapAttribute<'a, Self::Color>;

    /// Read VRAM region (0x8000 - 0x9fff)
    fn read_vram(&self, addr: u16, vram_bank0: &[u8; 0x2000]) -> u8;

    /// Write VRAM region (0x8000 - 0x9fff)
    fn write_vram(&mut self, addr: u16, v: u8, vram_bank0: &mut [u8; 0x2000]);

    /// Read BGP register (0xff47)
    fn read_bg_palette(&self) -> u8;

    /// Write BGP register (0xff47)
    fn write_bg_palette(&mut self, v: u8);

    /// Read OBP0 register (0xff48)
    fn read_obj_palette0(&self) -> u8;

    /// Write OBP0 register (0xff48)
    fn write_obj_palette0(&mut self, v: u8);

    /// Read OBP1 register (0xff49)
    fn read_obj_palette1(&self) -> u8;

    /// Write OBP1 register (0xff49)
    fn write_obj_palette1(&mut self, v: u8);

    /// Read VBK register (0xff4f)
    fn read_vram_bank_select(&self) -> u8;

    /// Write VBK register (0xff4f)
    fn select_vram_bank(&mut self, v: u8);

    /// Write BCPS/BGPI register (0xff68)
    fn select_bg_color_palette(&mut self, v: u8);

    /// Read BCPD/BGPD register (0xff69)
    fn read_bg_color_palette(&self) -> u8;

    /// Write BCPD/BGPD register (0xff69)
    fn write_bg_color_palette(&mut self, v: u8);

    /// Write OCPS/OBPI register (0xff6a)
    fn select_obj_color_palette(&mut self, v: u8);

    /// Read OCPD/OBPD register (0xff6b)
    fn read_obj_color_palette(&self) -> u8;

    /// Write OCPD/OBPD register (0xff6b)
    fn write_obj_color_palette(&mut self, v: u8);
}

pub struct GpuCgbExtension {
    vram: [u8; 0x2000],
    vram_select: usize,
    bg_color_palette: ColorPalette,
    obj_color_palette: ColorPalette,
}

impl Default for GpuCgbExtension {
    fn default() -> Self {
        Self {
            vram: [0; 0x2000],
            bg_color_palette: ColorPalette::new(),
            obj_color_palette: ColorPalette::new(),
            vram_select: 0,
        }
    }
}

impl CgbExt for GpuCgbExtension {
    type Color = Color;
    fn get_col_coli(
        &self,
        vram_bank0: &[u8; 0x2000],
        tiles: u16,
        tile: Point,
        tile_offset: Point,
        mapbase: u16,
    ) -> (Self::Color, usize) {
        let tbase = get_tile_base(tiles, mapbase, tile, vram_bank0);
        let ti = tile.x + tile.y * 32;
        let attr = read_vram_bank(mapbase + ti, &self.vram) as usize;

        let palette = &self.bg_color_palette.cols[attr & 0x7][..];
        let vram_bank = (attr >> 3) & 1;
        let xflip = attr & 0x20 != 0;
        let yflip = attr & 0x40 != 0;
        let priority = attr & 0x80 != 0;

        let tyoff = if yflip {
            7 - tile_offset.y
        } else {
            tile_offset.y
        };
        let txoff = if xflip {
            7 - tile_offset.x
        } else {
            tile_offset.x
        };

        assert!(!priority);

        let coli = get_tile_byte(
            tbase,
            Point { x: txoff, y: tyoff },
            if vram_bank == 0 {
                vram_bank0
            } else {
                &self.vram
            },
        );
        (palette[coli], coli)
    }

    fn get_window_col(
        &self,
        vram_bank0: &[u8; 0x2000],
        tiles: u16,
        tile: Point,
        tile_offset: Point,
        mapbase: u16,
    ) -> Self::Color {
        let tbase = get_tile_base(tiles, mapbase, tile, vram_bank0);
        let ti = tile.x + tile.y * 32;
        let attr = read_vram_bank(mapbase + ti, &self.vram) as usize;

        let palette = &self.bg_color_palette.cols[attr & 0x7][..];
        let vram_bank = (attr >> 3) & 1;

        let coli = get_tile_byte(
            tbase,
            tile_offset,
            if vram_bank == 0 {
                vram_bank0
            } else {
                &self.vram
            },
        );
        palette[coli]
    }

    fn get_sp_attr<'a>(
        &'a self,
        attr: u8,
        vram_bank0: &'a [u8; 0x2000],
    ) -> MapAttribute<'a, Self::Color> {
        let attr = attr as usize;

        MapAttribute {
            palette: self.obj_color_palette.cols[attr & 0x7],
            vram_bank: if (attr >> 3) & 1 == 0 {
                vram_bank0
            } else {
                &self.vram
            },
            xflip: attr & 0x20 != 0,
            yflip: attr & 0x40 != 0,
            priority: attr & 0x80 != 0,
        }
    }

    fn read_vram(&self, addr: u16, vram_bank0: &[u8; 0x2000]) -> u8 {
        read_vram_bank(
            addr,
            if self.vram_select == 0 {
                vram_bank0
            } else {
                &self.vram
            },
        )
    }

    fn write_vram(&mut self, addr: u16, v: u8, vram_bank0: &mut [u8; 0x2000]) {
        write_vram_bank(
            addr,
            v,
            if self.vram_select == 0 {
                vram_bank0
            } else {
                &mut self.vram
            },
        )
    }

    /// Read BGP register (0xff47)
    fn read_bg_palette(&self) -> u8 {
        panic!("Read bg palette in CGB mode")
    }

    /// Write BGP register (0xff47)
    fn write_bg_palette(&mut self, _: u8) {
        panic!("Write palette in CGB mode");
    }

    /// Read OBP0 register (0xff48)
    fn read_obj_palette0(&self) -> u8 {
        panic!("Read Object palette in CGB mode");
    }

    /// Write OBP0 register (0xff48)
    fn write_obj_palette0(&mut self, _: u8) {
        panic!("Write Object palette in CGB mode");
    }

    /// Read OBP1 register (0xff49)
    fn read_obj_palette1(&self) -> u8 {
        panic!("Read Object palette 1 in CGB mode");
    }

    /// Write OBP1 register (0xff49)
    fn write_obj_palette1(&mut self, _: u8) {
        panic!("Write Object palette in CGB mode");
    }

    /// Read VBK register (0xff4f)
    fn read_vram_bank_select(&self) -> u8 {
        self.vram_select as u8 & 0xfe
    }

    /// Write VBK register (0xff4f)
    fn select_vram_bank(&mut self, v: u8) {
        self.vram_select = v as usize & 1;
    }

    /// Write BCPS/BGPI register (0xff68)
    fn select_bg_color_palette(&mut self, v: u8) {
        self.bg_color_palette.select(v);
    }

    /// Read BCPD/BGPD register (0xff69)
    fn read_bg_color_palette(&self) -> u8 {
        self.bg_color_palette.read()
    }

    /// Write BCPD/BGPD register (0xff69)
    fn write_bg_color_palette(&mut self, v: u8) {
        self.bg_color_palette.write(v);
    }

    /// Write OCPS/OBPI register (0xff6a)
    fn select_obj_color_palette(&mut self, v: u8) {
        self.obj_color_palette.select(v);
    }

    /// Read OCPD/OBPD register (0xff6b)
    fn read_obj_color_palette(&self) -> u8 {
        self.obj_color_palette.read()
    }

    /// Write OCPD/OBPD register (0xff6b)
    fn write_obj_color_palette(&mut self, v: u8) {
        self.obj_color_palette.write(v);
    }
}

pub struct Dmg {
    bg_palette: [DmgColor; 4],
    obj_palette0: [DmgColor; 4],
    obj_palette1: [DmgColor; 4],
}

impl Default for Dmg {
    fn default() -> Self {
        Self {
            bg_palette: [
                DmgColor::White,
                DmgColor::LightGray,
                DmgColor::DarkGray,
                DmgColor::Black,
            ],
            obj_palette0: [
                DmgColor::White,
                DmgColor::LightGray,
                DmgColor::DarkGray,
                DmgColor::Black,
            ],
            obj_palette1: [
                DmgColor::White,
                DmgColor::LightGray,
                DmgColor::DarkGray,
                DmgColor::Black,
            ],
        }
    }
}

impl CgbExt for Dmg {
    type Color = DmgColor;

    fn get_col_coli(
        &self,
        vram_bank0: &[u8; 0x2000],
        tiles: u16,
        tile: Point,
        tile_offset: Point,
        mapbase: u16,
    ) -> (Self::Color, usize) {
        let tbase = get_tile_base(tiles, mapbase, tile, vram_bank0);
        let coli = get_tile_byte(tbase, tile_offset, vram_bank0);

        (self.bg_palette[coli], coli)
    }

    fn get_window_col(
        &self,
        vram_bank0: &[u8; 0x2000],
        tiles: u16,
        tile: Point,
        tile_offset: Point,
        mapbase: u16,
    ) -> Self::Color {
        let tbase = get_tile_base(tiles, mapbase, tile, vram_bank0);
        let coli = get_tile_byte(tbase, tile_offset, vram_bank0);

        self.bg_palette[coli]
    }

    fn get_sp_attr<'a>(
        &'a self,
        attr: u8,
        vram_bank0: &'a [u8; 0x2000],
    ) -> MapAttribute<'a, Self::Color> {
        let palette = if attr & 0x10 != 0 {
            self.obj_palette1
        } else {
            self.obj_palette0
        };

        MapAttribute {
            palette,
            xflip: attr & 0x20 != 0,
            yflip: attr & 0x40 != 0,
            priority: attr & 0x80 != 0,
            vram_bank: vram_bank0,
        }
    }

    fn read_vram(&self, addr: u16, vram_bank0: &[u8; 0x2000]) -> u8 {
        read_vram_bank(addr, vram_bank0)
    }

    fn write_vram(&mut self, addr: u16, v: u8, vram_bank0: &mut [u8; 0x2000]) {
        write_vram_bank(addr, v, vram_bank0)
    }

    /// Read BGP register (0xff47)
    fn read_bg_palette(&self) -> u8 {
        debug!("Read Bg palette");
        from_palette(self.bg_palette)
    }

    /// Write BGP register (0xff47)
    fn write_bg_palette(&mut self, v: u8) {
        self.bg_palette = to_palette(v);
        debug!("Bg palette updated: {:?}", self.bg_palette);
    }

    /// Read OBP0 register (0xff48)
    fn read_obj_palette0(&self) -> u8 {
        debug!("Read Object palette 0");
        from_palette(self.obj_palette0)
    }

    /// Write OBP0 register (0xff48)
    fn write_obj_palette0(&mut self, v: u8) {
        self.obj_palette0 = to_palette(v);
        debug!("Object palette 0 updated: {:?}", self.obj_palette0);
    }

    /// Read OBP1 register (0xff49)
    fn read_obj_palette1(&self) -> u8 {
        debug!("Read Object palette 1");
        from_palette(self.obj_palette1)
    }

    /// Write OBP1 register (0xff49)
    fn write_obj_palette1(&mut self, v: u8) {
        self.obj_palette1 = to_palette(v);
        debug!("Object palette 1 updated: {:?}", self.obj_palette1);
    }

    /// Read VBK register (0xff4f)
    fn read_vram_bank_select(&self) -> u8 {
        panic!("Read VRAM bank select in DMG mode");
    }

    /// Write VBK register (0xff4f)
    fn select_vram_bank(&mut self, _: u8) {
        panic!("Write VRAM bank select in DMG mode");
    }

    /// Write BCPS/BGPI register (0xff68)
    fn select_bg_color_palette(&mut self, _: u8) {
        panic!("Select BG Color palette in DMG mode");
    }

    /// Read BCPD/BGPD register (0xff69)
    fn read_bg_color_palette(&self) -> u8 {
        panic!("Read BG Color palette in DMG mode");
    }

    /// Write BCPD/BGPD register (0xff69)
    fn write_bg_color_palette(&mut self, _: u8) {
        panic!("Write BG Color palette in DMG mode");
    }

    /// Write OCPS/OBPI register (0xff6a)
    fn select_obj_color_palette(&mut self, _: u8) {
        panic!("Select Obj Color palette in DMG mode");
    }

    /// Read OCPD/OBPD register (0xff6b)
    fn read_obj_color_palette(&self) -> u8 {
        panic!("Read Obj Color palette in DMG mode");
    }

    /// Write OCPD/OBPD register (0xff6b)
    fn write_obj_color_palette(&mut self, _: u8) {
        panic!("Write BG Color palette in DMG mode");
    }
}

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy,  PartialEq, Eq, Default)]
    struct LcdControl: u8 {
        const ENABLE = 1 << 7;
        const WINMAP = 1 << 6;
        const WINENABLE = 1 << 5;
        const TILES = 1 << 4;
        const BGMAP = 1 << 3;
        const SPSIZE = 1 << 2;
        const SPENABLE = 1 << 1;
        const BGENABLE = 1;
    }
}

impl LcdControl {
    fn get_bgmap(self) -> u16 {
        if self.contains(Self::BGMAP) {
            0x9c00
        } else {
            0x9800
        }
    }

    fn get_tiles(self) -> u16 {
        if self.contains(Self::TILES) {
            0x8000
        } else {
            0x8800
        }
    }

    fn get_winmap(self) -> u16 {
        if self.contains(Self::WINMAP) {
            0x9c00
        } else {
            0x9800
        }
    }

    fn get_spsize(self) -> u8 {
        if self.contains(Self::SPSIZE) {
            16
        } else {
            8
        }
    }
}

pub struct Gpu<Ext: CgbExt> {
    clocks: usize,

    lyc_interrupt: bool,
    oam_interrupt: bool,
    vblank_interrupt: bool,
    hblank_interrupt: bool,
    mode: Mode,

    ly: u8,
    lyc: u8,
    scy: u8,
    scx: u8,

    wx: u8,
    wy: u8,

    lcd_control: LcdControl,

    vram: [u8; 0x2000],

    oam: [u8; 0xa0],

    hdma: Hdma,

    pub cgb_ext: Ext,
}

fn to_palette(p: u8) -> [DmgColor; 4] {
    [
        (p & 0x3).into(),
        ((p >> 2) & 0x3).into(),
        ((p >> 4) & 0x3).into(),
        ((p >> 6) & 0x3).into(),
    ]
}

fn from_palette(p: [DmgColor; 4]) -> u8 {
    assert_eq!(p.len(), 4);

    u8::from(p[0]) | u8::from(p[1]) << 2 | u8::from(p[2]) << 4 | u8::from(p[3]) << 6
}

pub struct MapAttribute<'a, C> {
    palette: [C; 4],
    vram_bank: &'a [u8; 0x2000],
    xflip: bool,
    yflip: bool,
    priority: bool,
}

struct ColorPalette {
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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DmgColor {
    #[default]
    White,
    LightGray,
    DarkGray,
    Black,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Color {
    Dmg(DmgColor),
    Rgb(u8, u8, u8),
}

impl Default for Color {
    fn default() -> Self {
        Color::Dmg(DmgColor::default())
    }
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
            Color::Dmg(dmg) => u32::from(dmg),
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

impl From<DmgColor> for u32 {
    fn from(c: DmgColor) -> u32 {
        match c {
            DmgColor::White => 0xdddddd,
            DmgColor::LightGray => 0xaaaaaa,
            DmgColor::DarkGray => 0x888888,
            DmgColor::Black => 0x555555,
        }
    }
}

impl From<Color> for u8 {
    fn from(c: Color) -> u8 {
        match c {
            Color::Dmg(dmg) => u8::from(dmg),
            _ => unreachable!(),
        }
    }
}

impl From<DmgColor> for u8 {
    fn from(c: DmgColor) -> u8 {
        match c {
            DmgColor::White => 0,
            DmgColor::LightGray => 1,
            DmgColor::DarkGray => 2,
            DmgColor::Black => 3,
        }
    }
}

impl From<u8> for Color {
    fn from(v: u8) -> Color {
        Color::Dmg(v.into())
    }
}

impl From<u8> for DmgColor {
    fn from(v: u8) -> DmgColor {
        match v {
            0 => DmgColor::White,
            1 => DmgColor::LightGray,
            2 => DmgColor::DarkGray,
            3 => DmgColor::Black,
            _ => unreachable!(),
        }
    }
}

struct Hdma {
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

    /// Write HDMA5 register (0xff55)
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

    /// Read HDMA5 register (0xff55)
    fn status(&self) -> u8 {
        self.len | if self.on { 0x80 } else { 0x00 }
    }

    fn run(&mut self, hblank: bool) -> Option<DmaRequest> {
        if !self.on {
            return None;
        }

        // H-blank mode runs only in hblank.
        if self.hblank && !hblank {
            return None;
        }

        let size = if self.hblank {
            // H-blank mode copies 16 bytes.
            0x10
        } else {
            // General mode copies all bytes at once.
            (self.len as u16 + 1) * 0x10
        };

        info!(
            "HDMA transfer: {:04x} -> {:04x} ({})",
            self.src_wip, self.dst_wip, size
        );

        let req = DmaRequest::new(self.src_wip, self.dst_wip, size);

        self.src_wip += size;
        self.dst_wip += size;
        let (rem, of) = self.len.overflowing_sub(1);

        self.len = if self.hblank { rem } else { 0xff };
        self.on = if self.hblank { !of } else { false };

        Some(req)
    }
}

impl<Ext: CgbExt> Default for Gpu<Ext> {
    fn default() -> Self {
        Self::new()
    }
}

pub type LineToDraw<C> = (u8, [C; VRAM_WIDTH]);

impl<Ext: CgbExt> Gpu<Ext> {
    pub fn new() -> Self {
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
            lcd_control: Default::default(),

            vram: [0; 0x2000],

            oam: [0; 0xa0],
            hdma: Hdma::new(),
            cgb_ext: Ext::default(),
        }
    }

    pub fn step(
        &mut self,
        time: usize,
        irq: &mut Irq,
    ) -> (Option<DmaRequest>, Option<LineToDraw<Ext::Color>>) {
        let clocks = self.clocks + time;

        let mut draw_line = None;

        let (clocks, mode) = match (self.mode, clocks) {
            (Mode::Oam, 80..) => (clocks - 80, Mode::Vram),
            (Mode::Vram, 172..) => {
                draw_line = self.draw();

                irq.request |= Ints::from_bits_retain(u8::from(self.hblank_interrupt)) & Ints::LCD;

                (clocks - 172, Mode::HBlank)
            }
            (Mode::HBlank, 204..) => {
                self.ly += 1;

                // ly becomes 144 before vblank interrupt
                if self.ly > 143 {
                    irq.request |= Ints::VBLANK;
                    irq.request |=
                        Ints::from_bits_retain(u8::from(self.vblank_interrupt)) & Ints::LCD;

                    (clocks - 204, Mode::VBlank)
                } else {
                    irq.request |= Ints::from_bits_retain(u8::from(self.oam_interrupt)) & Ints::LCD;

                    (clocks - 204, Mode::Oam)
                }
            }
            (Mode::VBlank, 456..) => {
                self.ly += 1;

                if self.ly > 153 {
                    self.ly = 0;

                    irq.request |= Ints::from_bits_retain(u8::from(self.oam_interrupt)) & Ints::LCD;

                    (clocks - 456, Mode::Oam)
                } else {
                    (clocks - 456, Mode::VBlank)
                }
            }
            (Mode::None, _) => (0, Mode::None),
            (mode, clock) => (clock, mode),
        };

        irq.request |=
            Ints::from_bits_retain(u8::from(self.lyc_interrupt && self.lyc == self.ly)) & Ints::LCD;

        let enter_hblank = self.mode != Mode::HBlank && mode == Mode::HBlank;

        self.clocks = clocks;
        self.mode = mode;

        (self.hdma.run(enter_hblank), draw_line)
    }

    fn draw(&self) -> Option<(u8, [Ext::Color; VRAM_WIDTH])> {
        let height = VRAM_HEIGHT;
        let width = VRAM_WIDTH;

        if self.ly >= height as u8 {
            return None;
        }

        let mut buf = [Ext::Color::default(); VRAM_WIDTH];
        let mut bgbuf = [0; VRAM_WIDTH];

        if self.lcd_control.contains(LcdControl::BGENABLE) {
            let mapbase = self.lcd_control.get_bgmap();

            let yy = (self.ly as u16 + self.scy as u16) % 256;
            let ty = yy / 8;
            let tyoff = yy % 8;

            for x in 0..width as u16 {
                let xx = (x + self.scx as u16) % 256;
                let tx = xx / 8;
                let txoff = xx % 8;

                let (col, coli) = self.cgb_ext.get_col_coli(
                    &self.vram,
                    self.lcd_control.get_tiles(),
                    Point { x: tx, y: ty },
                    Point { x: txoff, y: tyoff },
                    mapbase,
                );
                buf[x as usize] = col;
                bgbuf[x as usize] = coli;
            }
        }

        if self.lcd_control.contains(LcdControl::WINENABLE) {
            let mapbase = self.lcd_control.get_winmap();

            if self.ly >= self.wy {
                let yy = (self.ly - self.wy) as u16;
                let ty = yy / 8;
                let tyoff = yy % 8;

                for x in 0..width as u16 {
                    if x + 7 < self.wx as u16 {
                        continue;
                    }
                    let xx = x + 7 - self.wx as u16; // x - (wx - 7)
                    let tx = xx / 8;
                    let txoff = xx % 8;

                    buf[x as usize] = self.cgb_ext.get_window_col(
                        &self.vram,
                        self.lcd_control.get_tiles(),
                        Point { x: tx, y: ty },
                        Point { x: txoff, y: tyoff },
                        mapbase,
                    );
                }
            }
        }

        if self.lcd_control.contains(LcdControl::SPENABLE) {
            for i in 0..40 {
                let oam = i * 4;
                let ypos = self.oam[oam] as u16;
                let xpos = self.oam[oam + 1] as u16;
                let ti = self.oam[oam + 2];
                let attr = self.cgb_ext.get_sp_attr(self.oam[oam + 3], &self.vram);

                let ly = self.ly as u16;
                if ly + 16 < ypos {
                    // This sprite doesn't hit the current ly
                    continue;
                }
                let tyoff = ly + 16 - ypos; // ly - (ypos - 16)
                if tyoff >= u16::from(self.lcd_control.get_spsize()) {
                    // This sprite doesn't hit the current ly
                    continue;
                }
                let tyoff = if attr.yflip {
                    u16::from(self.lcd_control.get_spsize()) - 1 - tyoff
                } else {
                    tyoff
                };
                let ti = if self.lcd_control.get_spsize() == 16 {
                    if tyoff >= 8 {
                        ti | 1
                    } else {
                        ti & 0xfe
                    }
                } else {
                    ti
                };
                let tyoff = tyoff % 8;

                let tiles = 0x8000;

                for x in 0..width as u16 {
                    if x + 8 < xpos {
                        continue;
                    }
                    let txoff = x + 8 - xpos; // x - (xpos - 8)
                    if txoff >= 8 {
                        continue;
                    }
                    let txoff = if attr.xflip { 7 - txoff } else { txoff };

                    let tbase = tiles + ti as u16 * 16;

                    let coli = get_tile_byte(tbase, Point { x: txoff, y: tyoff }, attr.vram_bank);

                    if coli == 0 {
                        // Color index 0 means transparent
                        continue;
                    }

                    let col = attr.palette[coli];

                    let bgcoli = bgbuf[x as usize];

                    if attr.priority && bgcoli != 0 {
                        // If priority is lower than bg color 1-3, don't draw
                        continue;
                    }

                    buf[x as usize] = col;
                }
            }
        }

        Some((self.ly, buf))
    }

    /// Write CTRL register (0xff40)
    pub(crate) fn write_ctrl(&mut self, value: u8, irq: &mut Irq) {
        let value = LcdControl::from_bits_retain(value);

        if !self.lcd_control.contains(LcdControl::ENABLE) && value.contains(LcdControl::ENABLE) {
            info!("LCD enabled");
            self.clocks = 0;
            self.mode = Mode::HBlank;
        } else if self.lcd_control.contains(LcdControl::ENABLE)
            && !value.contains(LcdControl::ENABLE)
        {
            info!("LCD disabled");
            self.mode = Mode::None;
        }

        self.lcd_control = value;

        irq.request.remove(Ints::VBLANK);
    }

    /// Write STAT register (0xff41)
    pub(crate) fn write_status(&mut self, value: u8) {
        self.lyc_interrupt = value & 0x40 != 0;
        self.oam_interrupt = value & 0x20 != 0;
        self.vblank_interrupt = value & 0x10 != 0;
        self.hblank_interrupt = value & 0x08 != 0;

        debug!("LYC interrupt: {}", self.lyc_interrupt);
        debug!("OAM interrupt: {}", self.oam_interrupt);
        debug!("VBlank interrupt: {}", self.vblank_interrupt);
        debug!("HBlank interrupt: {}", self.hblank_interrupt);
    }

    // Read CTRL register (0xff40)
    pub(crate) fn read_ctrl(&self) -> u8 {
        self.lcd_control.bits()
    }

    // Write STAT register (0xff41)
    pub(crate) fn read_status(&self) -> u8 {
        let mut v = 0;
        v |= if self.lyc_interrupt { 0x40 } else { 0x00 };
        v |= if self.oam_interrupt { 0x20 } else { 0x00 };
        v |= if self.vblank_interrupt { 0x10 } else { 0x00 };
        v |= if self.hblank_interrupt { 0x08 } else { 0x00 };
        v |= if self.ly == self.lyc { 0x04 } else { 0x00 };
        v |= {
            let p: u8 = self.mode.into();
            p
        };
        trace!("Read Status: {:02x}", v);
        v
    }

    /// Read OAM region (0xfe00 - 0xfe9f)
    pub(crate) fn read_oam(&self, addr: u16) -> u8 {
        self.oam[addr as usize - 0xfe00]
    }

    /// Write OAM region (0xfe00 - 0xfe9f)
    pub(crate) fn write_oam(&mut self, addr: u16, v: u8) {
        self.oam[addr as usize - 0xfe00] = v;
    }

    /// Read SCY register (0xff42)
    pub(crate) fn read_scy(&self) -> u8 {
        self.scy
    }

    /// Write SCY register (0xff42)
    pub(crate) fn write_scy(&mut self, v: u8) {
        self.scy = v;
    }

    /// Read SCX register (0xff43)
    pub(crate) fn read_scx(&self) -> u8 {
        self.scx
    }

    /// Write SCX register (0xff43)
    pub(crate) fn write_scx(&mut self, v: u8) {
        self.scx = v;
    }

    /// Read LY register (0xff44)
    pub(crate) fn read_ly(&self) -> u8 {
        self.ly
    }

    /// Clear LY register (0xff44)
    pub(crate) fn clear_ly(&mut self) {
        self.ly = 0;
    }

    /// Read LYC register (0xff45)
    pub(crate) fn read_lyc(&self) -> u8 {
        self.lyc
    }

    /// Write LYC register (0xff45)
    pub(crate) fn write_lyc(&mut self, v: u8) {
        self.lyc = v;
    }

    /// Read WY register (0xff4a)
    pub(crate) fn read_wy(&self) -> u8 {
        self.wy
    }

    /// Write WY register (0xff4a)
    pub(crate) fn write_wy(&mut self, v: u8) {
        self.wy = v;
    }

    /// Read WX register (0xff4b)
    pub(crate) fn read_wx(&self) -> u8 {
        self.wx
    }

    /// Write WX register (0xff4b)
    pub(crate) fn write_wx(&mut self, v: u8) {
        self.wx = v;
    }

    /// Read HDMA1 register (0xff51)
    pub(crate) fn read_hdma_src_high(&self) -> u8 {
        self.hdma.src_high
    }

    /// Write HDMA1 register (0xff51)
    pub(crate) fn write_hdma_src_high(&mut self, v: u8) {
        self.hdma.src_high = v;
    }

    /// Read HDMA2 register (0xff52)
    pub(crate) fn read_hdma_src_low(&self) -> u8 {
        self.hdma.src_low
    }

    /// Write HDMA2 register (0xff52)
    pub(crate) fn write_hdma_src_low(&mut self, v: u8) {
        self.hdma.src_low = v;
    }

    /// Read HDMA3 register (0xff53)
    pub(crate) fn read_hdma_dst_high(&self) -> u8 {
        self.hdma.dst_high
    }

    /// Write HDMA3 register (0xff53)
    pub(crate) fn write_hdma_dst_high(&mut self, v: u8) {
        self.hdma.dst_high = v;
    }

    /// Read HDMA4 register (0xff54)
    pub(crate) fn read_hdma_dst_low(&self) -> u8 {
        self.hdma.dst_low
    }

    /// Write HDMA4 register (0xff54)
    pub(crate) fn write_hdma_dst_low(&mut self, v: u8) {
        self.hdma.dst_low = v;
    }

    /// Read HDMA5 register (0xff55)
    pub(crate) fn read_hdma_start(&self) -> u8 {
        self.hdma.status()
    }

    /// Write HDMA5 register (0xff55)
    pub(crate) fn write_hdma_start(&mut self, v: u8) {
        self.hdma.start(v);
    }

    /// Read BGP register (0xff47)
    pub(crate) fn read_bg_palette(&self) -> u8 {
        self.cgb_ext.read_bg_palette()
    }

    /// Write BGP register (0xff47)
    pub(crate) fn write_bg_palette(&mut self, v: u8) {
        self.cgb_ext.write_bg_palette(v)
    }

    /// Read OBP0 register (0xff48)
    pub(crate) fn read_obj_palette0(&self) -> u8 {
        self.cgb_ext.read_obj_palette0()
    }

    /// Write OBP0 register (0xff48)
    pub(crate) fn write_obj_palette0(&mut self, v: u8) {
        self.cgb_ext.write_obj_palette0(v)
    }

    /// Read OBP1 register (0xff49)
    pub(crate) fn read_obj_palette1(&self) -> u8 {
        self.cgb_ext.read_obj_palette1()
    }

    /// Write OBP1 register (0xff49)
    pub(crate) fn write_obj_palette1(&mut self, v: u8) {
        self.cgb_ext.write_obj_palette1(v)
    }

    /// Read VBK register (0xff4f)
    pub(crate) fn read_vram_bank_select(&self) -> u8 {
        self.cgb_ext.read_vram_bank_select()
    }

    /// Write VBK register (0xff4f)
    pub(crate) fn select_vram_bank(&mut self, v: u8) {
        self.cgb_ext.select_vram_bank(v)
    }

    /// Write BCPS/BGPI register (0xff68)
    pub(crate) fn select_bg_color_palette(&mut self, v: u8) {
        self.cgb_ext.select_bg_color_palette(v)
    }

    /// Read BCPD/BGPD register (0xff69)
    pub(crate) fn read_bg_color_palette(&self) -> u8 {
        self.cgb_ext.read_bg_color_palette()
    }

    /// Write BCPD/BGPD register (0xff69)
    pub(crate) fn write_bg_color_palette(&mut self, v: u8) {
        self.cgb_ext.write_bg_color_palette(v)
    }

    /// Write OCPS/OBPI register (0xff6a)
    pub(crate) fn select_obj_color_palette(&mut self, v: u8) {
        self.cgb_ext.select_obj_color_palette(v)
    }

    /// Read OCPD/OBPD register (0xff6b)
    pub(crate) fn read_obj_color_palette(&self) -> u8 {
        self.cgb_ext.read_obj_color_palette()
    }

    /// Write OCPD/OBPD register (0xff6b)
    pub(crate) fn write_obj_color_palette(&mut self, v: u8) {
        self.cgb_ext.write_obj_color_palette(v)
    }

    pub(crate) fn read_vram(&self, addr: u16) -> u8 {
        self.cgb_ext.read_vram(addr, &self.vram)
    }

    pub(crate) fn write_vram(&mut self, addr: u16, v: u8) {
        self.cgb_ext.write_vram(addr, v, &mut self.vram)
    }
}

fn get_tile_base(tiles: u16, mapbase: u16, tile: Point, vram_bank0: &[u8; 0x2000]) -> u16 {
    let ti = tile.x + tile.y * 32;
    let num = read_vram_bank(mapbase + ti, vram_bank0);

    if tiles == 0x8000 {
        tiles + num as u16 * 16
    } else {
        tiles + (0x800 + num as i8 as i16 * 16) as u16
    }
}

fn get_tile_byte(tilebase: u16, tile_offset: Point, bank: &[u8; 0x2000]) -> usize {
    let l = read_vram_bank(tilebase + tile_offset.y * 2, bank);
    let h = read_vram_bank(tilebase + tile_offset.y * 2 + 1, bank);

    let l = (l >> (7 - tile_offset.x)) & 1;
    let h = ((h >> (7 - tile_offset.x)) & 1) << 1;

    (h | l) as usize
}
fn read_vram_bank(addr: u16, bank: &[u8; 0x2000]) -> u8 {
    let off = addr as usize - 0x8000;
    bank[off]
}
fn write_vram_bank(addr: u16, value: u8, bank: &mut [u8; 0x2000]) {
    let off = addr as usize - 0x8000;
    bank[off] = value;
}
