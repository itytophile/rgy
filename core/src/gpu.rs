use core::convert::TryInto;

use crate::dma::DmaRequest;
use crate::hardware::{VRAM_HEIGHT, VRAM_WIDTH};
use crate::ic::{Ints, Irq};
use log::*;

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy,  PartialEq, Eq, Default)]
    struct LcdStatus: u8 {
        const LYC_INT = 1 << 6;
        const OAM_INT = 1 << 5;
        const VBLANK_INT = 1 << 4;
        const HBLANK_INT = 1 << 3;
    }
}

// https://gbdev.io/pandocs/Rendering.html#ppu-modes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    OamScan,
    Drawing,
    HBlank,
    VBlank,
    None,
}

impl From<Mode> for u8 {
    fn from(v: Mode) -> u8 {
        match v {
            Mode::HBlank => 0,
            Mode::VBlank => 1,
            Mode::OamScan => 2,
            Mode::Drawing => 3,
            Mode::None => 0,
        }
    }
}

impl From<u8> for Mode {
    fn from(v: u8) -> Mode {
        match v {
            0 => Mode::HBlank,
            1 => Mode::VBlank,
            2 => Mode::OamScan,
            3 => Mode::Drawing,
            _ => Mode::None,
        }
    }
}

#[derive(Clone, Copy)]
pub struct Point {
    x: u8,
    y: u8,
}

pub trait CgbExt: Default {
    type Color: Default + Copy;

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

    fn get_scanline_after_offset(
        &self,
        scx: u8,
        y: u8,
        vram_bank0: &[u8; 0x2000],
        tiles: u16,
        mapbase: u16,
        buf: &mut [Self::Color; VRAM_WIDTH as usize],
        bgbuf: Option<&mut [u8; VRAM_WIDTH as usize]>,
    );
}

pub struct GpuCgbExtension {
    vram: [u8; 0x2000],
    vram_select: u8,
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

    fn get_sp_attr<'a>(
        &'a self,
        attr: u8,
        vram_bank0: &'a [u8; 0x2000],
    ) -> MapAttribute<'a, Self::Color> {
        MapAttribute {
            palette: self.obj_color_palette.cols[usize::from(attr & 0x7)],
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
        self.vram_select & 0xfe
    }

    /// Write VBK register (0xff4f)
    fn select_vram_bank(&mut self, v: u8) {
        self.vram_select = v & 1;
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

    fn get_scanline_after_offset(
        &self,
        offset: u8,
        y: u8,
        vram_bank0: &[u8; 0x2000],
        tiles: u16,
        mapbase: u16,
        buf: &mut [Self::Color; VRAM_WIDTH as usize],
        bgbuf: Option<&mut [u8; VRAM_WIDTH as usize]>,
    ) {
        // (scx / 8..=u8::MAX / 8)
        //     .chain(0..) // we don't care about the upper limit because we call take() later anyway
        //     .flat_map(move |tx| {
        //         let tbase = get_tile_base(tiles, mapbase, Point { x: tx, y: y / 8 }, vram_bank0);
        //         let ti = u16::from(tx * 8) + u16::from(y) * 32;
        //         let attr = read_vram_bank(mapbase + ti, &self.vram);

        //         let palette = &self.bg_color_palette.cols[usize::from(attr & 0x7)][..];
        //         let vram_bank = (attr >> 3) & 1;

        //         let line = get_tile_line(
        //             tbase,
        //             y % 8,
        //             if vram_bank == 0 {
        //                 vram_bank0
        //             } else {
        //                 &self.vram
        //             },
        //         );
        //         (0u8..8u8).map(move |pixel_in_line| {
        //             let coli = get_color_id_from_tile_line(line, pixel_in_line);
        //             (palette[usize::from(coli)], coli)
        //         })
        //     })
        //     .skip(usize::from(scx % 8))
        //     .take(usize::from(VRAM_WIDTH))
        todo!()
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

    fn get_scanline_after_offset(
        &self,
        scx: u8,
        y: u8,
        vram_bank0: &[u8; 0x2000],
        tiles: u16,
        mapbase: u16,
        buf: &mut [Self::Color; VRAM_WIDTH as usize],
        mut bgbuf: Option<&mut [u8; VRAM_WIDTH as usize]>,
    ) {
        // thanks https://github.com/deltabeard/Peanut-GB/blob/4596d56ddb85a1aa45b1197c77f05e236a23bd94/peanut_gb.h#L1465
        let mut tbase = get_tile_base(
            tiles,
            mapbase,
            Point {
                x: (VRAM_WIDTH - 1).wrapping_add(scx) / 8,
                y: y / 8,
            },
            vram_bank0,
        );
        let mut line = get_tile_line(tbase, y % 8, vram_bank0);
        let mut offset = (8 - (scx % 8)) % 8;
        line[0] >>= offset;
        line[1] >>= offset;
        for i in (0..VRAM_WIDTH).rev() {
            if offset == 8 {
                tbase = get_tile_base(
                    tiles,
                    mapbase,
                    Point {
                        x: i.wrapping_add(scx) / 8,
                        y: y / 8,
                    },
                    vram_bank0,
                );
                line = get_tile_line(tbase, y % 8, vram_bank0);
                offset = 0;
            }
            let coli = (line[0] & 1) | ((line[1] & 1) << 1);
            buf[usize::from(i)] = self.bg_palette[usize::from(coli)];
            if let Some(b) = bgbuf {
                b[usize::from(i)] = coli;
                bgbuf = Some(b);
            }

            line[0] >>= 1;
            line[1] >>= 1;
            offset += 1;
        }
    }
}

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy,  PartialEq, Eq, Default)]
    struct LcdControl: u8 {
        const LCD_PPU_ENABLE = 1 << 7;
        const WINDOW_TILE_MAP = 1 << 6;
        const WINDOW_ENABLE = 1 << 5;
        const BG_AND_WINDOW_TILES = 1 << 4;
        const BG_TILE_MAP = 1 << 3;
        const OBJ_SIZE = 1 << 2;
        const OBJ_ENABLE = 1 << 1;
        const BG_AND_WINDOW_ENABLE = 1;
    }
}

impl LcdControl {
    fn get_bgmap(self) -> u16 {
        if self.contains(Self::BG_TILE_MAP) {
            0x9c00
        } else {
            0x9800
        }
    }

    fn get_bg_and_window_tile_area(self) -> u16 {
        if self.contains(Self::BG_AND_WINDOW_TILES) {
            0x8000
        } else {
            0x8800
        }
    }

    fn get_winmap(self) -> u16 {
        if self.contains(Self::WINDOW_TILE_MAP) {
            0x9c00
        } else {
            0x9800
        }
    }

    fn get_spsize(self) -> u8 {
        if self.contains(Self::OBJ_SIZE) {
            16
        } else {
            8
        }
    }
}

pub struct Gpu<Ext: CgbExt> {
    clocks: usize,

    lcd_status: LcdStatus,
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
        self.index = usize::from(value) & 0x3f;
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
    let v = u32::from(v);

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
            self.src_wip = (u16::from(self.src_high) << 8 | u16::from(self.src_low)) & !0x000f;
            self.dst_wip =
                (u16::from(self.dst_high) << 8 | u16::from(self.dst_low)) & !0xe00f | 0x8000;
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
            (u16::from(self.len) + 1) * 0x10
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

pub type LineToDraw<C> = (u8, [C; VRAM_WIDTH as usize]);

impl<Ext: CgbExt> Gpu<Ext> {
    pub fn new() -> Self {
        Self {
            clocks: 0,
            lcd_status: LcdStatus::empty(),
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
            (Mode::OamScan, 80..) => (clocks - 80, Mode::Drawing),
            (Mode::Drawing, 172..) => {
                draw_line = self.draw();

                if self.lcd_status.contains(LcdStatus::HBLANK_INT) {
                    irq.request |= Ints::LCD
                }

                (clocks - 172, Mode::HBlank)
            }
            (Mode::HBlank, 204..) => {
                self.ly += 1;

                // ly becomes 144 before vblank interrupt
                if self.ly > 143 {
                    irq.request |= Ints::VBLANK;
                    if self.lcd_status.contains(LcdStatus::VBLANK_INT) {
                        irq.request |= Ints::LCD
                    }

                    (clocks - 204, Mode::VBlank)
                } else {
                    if self.lcd_status.contains(LcdStatus::OAM_INT) {
                        irq.request |= Ints::LCD
                    }

                    (clocks - 204, Mode::OamScan)
                }
            }
            (Mode::VBlank, 456..) => {
                self.ly += 1;

                if self.ly > 153 {
                    self.ly = 0;

                    if self.lcd_status.contains(LcdStatus::OAM_INT) {
                        irq.request |= Ints::LCD;
                    }

                    (clocks - 456, Mode::OamScan)
                } else {
                    (clocks - 456, Mode::VBlank)
                }
            }
            (Mode::None, _) => (0, Mode::None),
            (mode, clock) => (clock, mode),
        };

        if self.lcd_status.contains(LcdStatus::LYC_INT) && self.lyc == self.ly {
            irq.request |= Ints::LCD;
        }

        let enter_hblank = self.mode != Mode::HBlank && mode == Mode::HBlank;

        self.clocks = clocks;
        self.mode = mode;

        (self.hdma.run(enter_hblank), draw_line)
    }

    fn draw(&self) -> Option<(u8, [Ext::Color; VRAM_WIDTH as usize])> {
        if self.ly >= VRAM_HEIGHT {
            return None;
        }

        let mut buf = [Ext::Color::default(); VRAM_WIDTH as usize];
        let mut bgbuf = [0u8; VRAM_WIDTH as usize];

        if self.lcd_control.contains(LcdControl::BG_AND_WINDOW_ENABLE) {
            self.when_bg_and_window_enable(&mut buf, &mut bgbuf);
            // https://gbdev.io/pandocs/LCDC.html#non-cgb-mode-dmg-sgb-and-cgb-in-compatibility-mode-bg-and-window-display
            // When Bit 0 [LcdControl::BG_AND_WINDOW_ENABLE] is cleared, both background and window become blank (white),
            // and the Window Display Bit [LcdControl::WINDOW_ENABLE] is ignored in that case.
            // Only objects may still be displayed (if enabled in Bit 1).
            if self.lcd_control.contains(LcdControl::WINDOW_ENABLE) {
                self.when_window_enable(&mut buf);
            }
        }

        if self.lcd_control.contains(LcdControl::OBJ_ENABLE) {
            self.when_obj_enable(&bgbuf, &mut buf);
        }

        Some((self.ly, buf))
    }

    fn when_bg_and_window_enable(
        &self,
        buf: &mut [<Ext as CgbExt>::Color; 160],
        bgbuf: &mut [u8; 160],
    ) {
        self.cgb_ext.get_scanline_after_offset(
            self.scx,
            self.ly.wrapping_add(self.scy),
            &self.vram,
            self.lcd_control.get_bg_and_window_tile_area(),
            self.lcd_control.get_bgmap(),
            buf,
            Some(bgbuf),
        );
    }

    fn when_window_enable(&self, buf: &mut [<Ext as CgbExt>::Color; 160]) {
        if self.ly >= self.wy {
            self.cgb_ext.get_scanline_after_offset(
                self.wx.saturating_sub(7),
                self.ly - self.wy,
                &self.vram,
                self.lcd_control.get_bg_and_window_tile_area(),
                self.lcd_control.get_winmap(),
                buf,
                None,
            );
        }
    }

    fn when_obj_enable(&self, bgbuf: &[u8; 160], buf: &mut [<Ext as CgbExt>::Color; 160]) {
        for oam in self.oam.chunks(4) {
            let ypos = oam[0];

            if self.ly + 16 < ypos {
                // This sprite doesn't hit the current ly
                continue;
            }

            let tyoff = self.ly + 16 - ypos; // ly - (ypos - 16)

            if tyoff >= self.lcd_control.get_spsize() {
                // This sprite doesn't hit the current ly
                continue;
            }

            let attr = self.cgb_ext.get_sp_attr(oam[3], &self.vram);

            let tyoff = if attr.yflip {
                self.lcd_control.get_spsize() - 1 - tyoff
            } else {
                tyoff
            };

            let ti = oam[2];

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

            let xpos = oam[1];

            if xpos == 0 || xpos >= VRAM_WIDTH + 8 {
                // the object is off-screen
                // https://gbdev.io/pandocs/OAM.html#byte-1--x-position
                continue;
            }

            let tbase = tiles + u16::from(ti) * 16;
            let mut line = get_tile_line(tbase, tyoff, attr.vram_bank);

            if attr.xflip {
                // we have to shift only if the sprite is partially off-screen (screen left side)
                let offset = 8u8.saturating_sub(xpos);
                line[0] >>= offset;
                line[1] >>= offset;
                // we don't need to call .rev() because we want to keep the "natural" flip of the right to left read
                for x in xpos.saturating_sub(8)..VRAM_WIDTH.min(xpos) {
                    let coli = (line[0] & 1) | ((line[1] & 1) << 1);
                    line[0] >>= 1;
                    line[1] >>= 1;

                    if coli == 0 {
                        // Color index 0 means transparent
                        continue;
                    }

                    let col = attr.palette[usize::from(coli)];

                    let bgcoli = bgbuf[usize::from(x)];

                    if attr.priority && bgcoli != 0 {
                        // If priority is lower than bg color 1-3, don't draw
                        continue;
                    }

                    buf[usize::from(x)] = col;
                }
            } else {
                // we have to shift only if the sprite is partially off-screen (screen right side)
                let offset = xpos.saturating_sub(VRAM_WIDTH);
                line[0] >>= offset;
                line[1] >>= offset;
                for x in (xpos.saturating_sub(8)..VRAM_WIDTH.min(xpos)).rev() {
                    let coli = (line[0] & 1) | ((line[1] & 1) << 1);
                    line[0] >>= 1;
                    line[1] >>= 1;

                    if coli == 0 {
                        // Color index 0 means transparent
                        continue;
                    }

                    let col = attr.palette[usize::from(coli)];

                    let bgcoli = bgbuf[usize::from(x)];

                    if attr.priority && bgcoli != 0 {
                        // If priority is lower than bg color 1-3, don't draw
                        continue;
                    }

                    buf[usize::from(x)] = col;
                }
            }
        }
    }

    /// Write CTRL register (0xff40)
    pub(crate) fn write_ctrl(&mut self, value: u8, irq: &mut Irq) {
        let value = LcdControl::from_bits_retain(value);

        if !self.lcd_control.contains(LcdControl::LCD_PPU_ENABLE)
            && value.contains(LcdControl::LCD_PPU_ENABLE)
        {
            info!("LCD enabled");
            self.clocks = 0;
            self.mode = Mode::HBlank;
        } else if self.lcd_control.contains(LcdControl::LCD_PPU_ENABLE)
            && !value.contains(LcdControl::LCD_PPU_ENABLE)
        {
            info!("LCD disabled");
            self.mode = Mode::None;
        }

        self.lcd_control = value;

        irq.request.remove(Ints::VBLANK);
    }

    /// Write STAT register (0xff41)
    pub(crate) fn write_status(&mut self, value: u8) {
        self.lcd_status = LcdStatus::from_bits_truncate(value);
    }

    // Read CTRL register (0xff40)
    pub(crate) fn read_ctrl(&self) -> u8 {
        self.lcd_control.bits()
    }

    // Write STAT register (0xff41)
    pub(crate) fn read_status(&self) -> u8 {
        self.lcd_status.bits() | u8::from(self.mode)
    }

    /// Read OAM region (0xfe00 - 0xfe9f)
    pub(crate) fn read_oam(&self, addr: u16) -> u8 {
        self.oam[usize::from(addr) - 0xfe00]
    }

    /// Write OAM region (0xfe00 - 0xfe9f)
    pub(crate) fn write_oam(&mut self, addr: u16, v: u8) {
        self.oam[usize::from(addr) - 0xfe00] = v;
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
    let ti = u16::from(tile.x) + u16::from(tile.y) * 32;
    let num = read_vram_bank(mapbase + ti, vram_bank0);

    if tiles == 0x8000 {
        tiles + u16::from(num) * 16
    } else {
        tiles + (0x800 + i16::from(num as i8) * 16) as u16
    }
}

/// https://gbdev.io/pandocs/Tile_Data.html#vram-tile-data
///
/// Each tile occupies 16 bytes, where each line is represented by 2 bytes
fn get_tile_line(tilebase: u16, y_offset: u8, bank: &[u8; 0x2000]) -> [u8; 2] {
    let off = usize::from(tilebase + u16::from(y_offset) * 2 - 0x8000);
    bank[off..=off + 1].try_into().unwrap()
}

fn get_color_id_from_tile_line(line: [u8; 2], x_offset: u8) -> u8 {
    let l = (line[0] >> (7 - x_offset)) & 1;
    let h = ((line[1] >> (7 - x_offset)) & 1) << 1;
    h | l
}

fn read_vram_bank(addr: u16, bank: &[u8; 0x2000]) -> u8 {
    let off = addr - 0x8000;
    bank[usize::from(off)]
}
fn write_vram_bank(addr: u16, value: u8, bank: &mut [u8; 0x2000]) {
    let off = addr - 0x8000;
    bank[usize::from(off)] = value;
}
