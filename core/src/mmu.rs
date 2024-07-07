use crate::{cgb::Cgb, device::IoHandler, dma::Dma, gpu::{Gpu, Mode}, ic::{Ic, Irq}, joypad::Joypad, mbc::Mbc, serial::Serial, sound::{MixerStream, Sound}, timer::Timer, VRAM_HEIGHT, VRAM_WIDTH};

/// The variants to control memory read access from the CPU.
pub enum MemRead {
    /// Replaces the value passed from the memory to the CPU.
    Replace(u8),
    /// Shows the actual value passed from the memory to the CPU.
    PassThrough,
}

/// The variants to control memory write access from the CPU.
pub enum MemWrite {
    /// Replaces the value to be written by the CPU to the memory.
    Replace(u8),
    /// Allows to write the original value from the CPU to the memory.
    PassThrough,
    /// Discard the write access from the CPU.
    Block,
}

/// The memory management unit (MMU)
///
/// This unit holds a memory byte array which represents address space of the memory.
/// It provides the logic to intercept access from the CPU to the memory byte array,
/// and to modify the memory access behaviour.
pub struct Mmu<'a, 'b> {
    pub inner: &'b mut MmuWithoutMixerStream,
    pub mixer_stream: &'b mut MixerStream,
    pub irq: &'b mut Irq,
    pub handlers: &'b mut MemHandlers<'a>,
}

pub struct MmuWithoutMixerStream {
    pub ram: [u8; 0x10000],
}

impl<'a> Default for MmuWithoutMixerStream {
    fn default() -> Self {
        Self { ram: [0; 0x10000] }
    }
}

pub struct MemHandlers<'a> {
    pub cgb: Cgb,
    pub mbc: Mbc<'a>,
    pub sound: Sound,
    pub dma: Dma,
    pub gpu: Gpu,
    pub ic: Ic,
    pub joypad: Joypad<'a>,
    pub timer: Timer,
    pub serial: Serial<'a>
}

impl<'a, 'b> Mmu<'a, 'b> {
    fn on_read(&mut self, addr: u16) -> Option<MemRead> {
        match addr {
            0xc000..=0xdfff | 0xff4d | 0xff56 | 0xff70 => {
                Some(self.handlers.cgb.on_read(addr, self.mixer_stream, self.irq))
            }
            0x0000..=0x7fff | 0xff50 | 0xa000..=0xbfff => {
                Some(self.handlers.mbc.on_read(addr, self.mixer_stream, self.irq))
            }
            0xff10..=0xff3f => Some(self.handlers.sound.on_read(addr, self.mixer_stream, self.irq)),
            0xff46 => Some(self.handlers.dma.on_read(addr, self.mixer_stream, self.irq)),
            0x8000..=0x9fff
            | 0xff40..=0xff45
            | 0xff47..=0xff4b
            | 0xff4f
            | 0xff51..=0xff55
            | 0xff68..=0xff6b => Some(self.handlers.gpu.on_read(addr, self.mixer_stream, self.irq)),
            0xff0f | 0xffff => Some(self.handlers.ic.on_read(addr, self.mixer_stream, self.irq)),
            0xff00 => Some(
                self.handlers
                    .joypad
                    .on_read(addr, self.mixer_stream, self.irq),
            ),
            0xff04..=0xff07 => Some(self.handlers.timer.on_read(addr, self.mixer_stream, self.irq)),
            0xff01..=0xff02 => Some(
                self.handlers
                    .serial
                    .on_read(addr, self.mixer_stream, self.irq),
            ),
            _ => None,
        }
    }

    fn on_write(&mut self, addr: u16, v: u8) -> Option<MemWrite> {
        match addr {
            0xc000..=0xdfff | 0xff4d | 0xff56 | 0xff70 => Some(self.handlers.cgb.on_write(
                addr,
                v,
                self.mixer_stream,
                self.irq,
            )),
            0x0000..=0x7fff | 0xff50 | 0xa000..=0xbfff => Some(self.handlers.mbc.on_write(
                addr,
                v,
                self.mixer_stream,
                self.irq,
            )),
            0xff10..=0xff3f => Some(self.handlers.sound.on_write(
                addr,
                v,
                self.mixer_stream,
                self.irq,
            )),
            0xff46 => Some(
                self.handlers
                    .dma
                    .on_write(addr, v, self.mixer_stream, self.irq),
            ),
            0x8000..=0x9fff
            | 0xff40..=0xff45
            | 0xff47..=0xff4b
            | 0xff4f
            | 0xff51..=0xff55
            | 0xff68..=0xff6b => Some(self.handlers.gpu.on_write(
                addr,
                v,
                self.mixer_stream,
                self.irq,
            )),
            0xff0f | 0xffff => Some(
                self.handlers
                    .ic
                    .on_write(addr, v, self.mixer_stream, self.irq),
            ),
            0xff00 => Some(
                self.handlers
                    .joypad
                    .on_write(addr, v, self.mixer_stream, self.irq),
            ),
            0xff04..=0xff07 => Some(self.handlers.timer.on_write(
                addr,
                v,
                self.mixer_stream,
                self.irq,
            )),
            0xff01..=0xff02 => Some(self.handlers.serial.on_write(
                addr,
                v,
                self.mixer_stream,
                self.irq,
            )),
            _ => None,
        }
    }
    /// Reads one byte from the given address in the memory.
    pub fn get8(&mut self, addr: u16) -> u8 {
        match self.on_read(addr) {
            Some(MemRead::Replace(alt)) => return alt,
            _ => {}
        }
        if (0xe000..=0xfdff).contains(&addr) {
            // echo ram
            self.inner.ram[addr as usize - 0x2000]
        } else {
            self.inner.ram[addr as usize]
        }
    }

    /// Writes one byte at the given address in the memory.
    pub fn set8(&mut self, addr: u16, v: u8) {
        match self.on_write(addr, v) {
            Some(MemWrite::Replace(alt)) => {
                self.inner.ram[addr as usize] = alt;
                return;
            }
            Some(MemWrite::Block) => return,
            _ => {}
        }

        if (0xe000..=0xfdff).contains(&addr) {
            // echo ram
            self.inner.ram[addr as usize - 0x2000] = v
        } else {
            self.inner.ram[addr as usize] = v
        }
    }

    /// Reads two bytes from the given addresss in the memory.
    pub fn get16(&mut self, addr: u16) -> u16 {
        u16::from_le_bytes([self.get8(addr), self.get8(addr + 1)])
    }

    /// Writes two bytes at the given address in the memory.
    pub fn set16(&mut self, addr: u16, v: u16) {
        self.set8(addr, v as u8);
        self.set8(addr + 1, (v >> 8) as u8);
    }

    pub fn dma_step(&mut self) {
        if self.handlers.dma.on {
            assert!(self.handlers.dma.src <= 0x80 || self.handlers.dma.src >= 0x9f);

            let src = (self.handlers.dma.src as u16) << 8;
            for i in 0..0xa0 {
                let get = self.get8(src + i);
                self.set8(0xfe00 + i, get);
            }

            self.handlers.dma.on = false;
        }
    }

    pub fn gpu_step(&mut self, time: usize) -> Option<(u8, [u32; VRAM_WIDTH])>{
        let clocks = self.handlers.gpu.clocks + time;

        let mut line_to_draw = None;

        let (clocks, mode) = match &self.handlers.gpu.mode {
            Mode::OAM => {
                if clocks >= 80 {
                    (0, Mode::VRAM)
                } else {
                    (clocks, Mode::OAM)
                }
            }
            Mode::VRAM => {
                if clocks >= 172 {
                    line_to_draw = self.gpu_draw();
                    self.gpu_hdma_run();

                    if self.handlers.gpu.hblank_interrupt {
                        self.irq.lcd(true);
                    }

                    (0, Mode::HBlank)
                } else {
                    (clocks, Mode::VRAM)
                }
            }
            Mode::HBlank => {
                if clocks >= 204 {
                    self.handlers.gpu.ly += 1;

                    // ly becomes 144 before vblank interrupt
                    if self.handlers.gpu.ly > 143 {
                        self.irq.vblank(true);

                        if self.handlers.gpu.vblank_interrupt {
                            self.irq.lcd(true);
                        }

                        (0, Mode::VBlank)
                    } else {
                        if self.handlers.gpu.oam_interrupt {
                            self.irq.lcd(true);
                        }

                        (0, Mode::OAM)
                    }
                } else {
                    (clocks, Mode::HBlank)
                }
            }
            Mode::VBlank => {
                if clocks >= 456 {
                    self.handlers.gpu.ly += 1;

                    if self.handlers.gpu.ly > 153 {
                        self.handlers.gpu.ly = 0;

                        if self.handlers.gpu.oam_interrupt {
                            self.irq.lcd(true);
                        }

                        (0, Mode::OAM)
                    } else {
                        (0, Mode::VBlank)
                    }
                } else {
                    (clocks, Mode::VBlank)
                }
            }
            Mode::None => (0, Mode::None),
        };

        if self.handlers.gpu.lyc_interrupt && self.handlers.gpu.lyc == self.handlers.gpu.ly {
            self.irq.lcd(true);
        }

        self.handlers.gpu.clocks = clocks;
        self.handlers.gpu.mode = mode;

        line_to_draw
    }

    pub fn gpu_hdma_run(&mut self) {
        if let Some((dst, src, size)) = self.handlers.gpu.hdma.run() {
            for i in 0..size {
                let value = self.get8(src + i);
                self.handlers.gpu.write_vram(dst + i, value, self.handlers.gpu.vram_select);
            }
        }
    }


    pub fn gpu_draw(&mut self) -> Option<(u8, [u32; VRAM_WIDTH])> {
        if self.handlers.gpu.ly >= VRAM_HEIGHT as u8 {
            return None;
        }

        let mut buf = [0; VRAM_WIDTH];
        let mut bgbuf = [0; VRAM_WIDTH];

        if self.handlers.gpu.bgenable {
            let mapbase = self.handlers.gpu.bgmap;

            let yy = (self.handlers.gpu.ly as u16 + self.handlers.gpu.scy as u16) % 256;
            let ty = yy / 8;
            let tyoff = yy % 8;

            for x in 0..VRAM_WIDTH as u16 {
                let xx = (x + self.handlers.gpu.scx as u16) % 256;
                let tx = xx / 8;
                let txoff = xx % 8;

                let tbase = self.handlers.gpu.get_tile_base(mapbase, tx, ty);
                let tattr = self.handlers.gpu.get_tile_attr(mapbase, tx, ty);

                let tyoff = if tattr.yflip { 7 - tyoff } else { tyoff };
                let txoff = if tattr.xflip { 7 - txoff } else { txoff };

                #[cfg(feature = "color")]
                {
                    assert_eq!(tattr.priority, false);
                }

                let coli = self.handlers.gpu.get_tile_byte(tbase, txoff, tyoff, tattr.vram_bank);
                let col = tattr.palette[coli].into();

                buf[x as usize] = col;
                bgbuf[x as usize] = coli;
            }
        }

        if self.handlers.gpu.winenable {
            let mapbase = self.handlers.gpu.winmap;

            if self.handlers.gpu.ly >= self.handlers.gpu.wy {
                let yy = (self.handlers.gpu.ly - self.handlers.gpu.wy) as u16;
                let ty = yy / 8;
                let tyoff = yy % 8;

                for x in 0..VRAM_WIDTH as u16 {
                    if x + 7 < self.handlers.gpu.wx as u16 {
                        continue;
                    }
                    let xx = x + 7 - self.handlers.gpu.wx as u16; // x - (wx - 7)
                    let tx = xx / 8;
                    let txoff = xx % 8;

                    let tbase = self.handlers.gpu.get_tile_base(mapbase, tx, ty);
                    let tattr = self.handlers.gpu.get_tile_attr(mapbase, tx, ty);

                    let coli = self.handlers.gpu.get_tile_byte(tbase, txoff, tyoff, tattr.vram_bank);
                    let col = tattr.palette[coli].into();

                    buf[x as usize] = col;
                }
            }
        }

        if self.handlers.gpu.spenable {
            for i in 0..40 {
                let oam = 0xfe00 + i * 4;
                let ypos = self.get8(oam) as u16;
                let xpos = self.get8(oam + 1) as u16;
                let ti = self.get8(oam + 2);
                let attr = self.get8(oam + 3);
                let attr = self.handlers.gpu.get_sp_attr(attr);

                let ly = self.handlers.gpu.ly as u16;
                if ly + 16 < ypos {
                    // This sprite doesn't hit the current ly
                    continue;
                }
                let tyoff = ly + 16 - ypos; // ly - (ypos - 16)
                if tyoff >= self.handlers.gpu.spsize {
                    // This sprite doesn't hit the current ly
                    continue;
                }
                let tyoff = if attr.yflip {
                    self.handlers.gpu.spsize - 1 - tyoff
                } else {
                    tyoff
                };
                let ti = if self.handlers.gpu.spsize == 16 {
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

                for x in 0..VRAM_WIDTH as u16 {
                    if x + 8 < xpos {
                        continue;
                    }
                    let txoff = x + 8 - xpos; // x - (xpos - 8)
                    if txoff >= 8 {
                        continue;
                    }
                    let txoff = if attr.xflip { 7 - txoff } else { txoff };

                    let tbase = tiles + ti as u16 * 16;

                    let coli = self.handlers.gpu.get_tile_byte(tbase, txoff, tyoff, attr.vram_bank);

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

                    buf[x as usize] = col.into();
                }
            }
        }

        Some((self.handlers.gpu.ly, buf))
    }
}
