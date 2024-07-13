use crate::apu::mixer::MixerStream;
use crate::apu::Apu;
use crate::cgb::Cgb;
use crate::cpu::Sys;
use crate::dma::{Dma, DmaRequest};
use crate::gpu::{self, Gpu};
use crate::hram::Hram;
use crate::ic::{Ic, Irq};
use crate::joypad::Joypad;
use crate::mbc::Mbc;
use crate::serial::Serial;
use crate::timer::Timer;
use crate::wram::{self, Wram};
use crate::Hardware;
use log::*;

pub enum DmgMode {}
pub enum CgbMode {}

impl GameboyMode for CgbMode {
    type Wram = wram::WramCgbExtension;

    type Gpu = gpu::GpuCgbExtension;
}

impl GameboyMode for DmgMode {
    type Wram = ();

    type Gpu = gpu::Dmg;
}

pub trait GameboyMode {
    type Wram: wram::CgbExt;
    type Gpu: gpu::CgbExt;
}

pub struct Peripherals<'a, H, GB: GameboyMode> {
    wram: Wram<GB::Wram>,
    hram: Hram,
    gpu: Gpu<GB::Gpu>,
    mbc: Mbc<'a>,
    timer: Timer,
    ic: Ic,
    serial: Serial,
    joypad: Joypad,
    apu: Apu,
    dma: Dma,
    cgb: Cgb,
    irq: Irq,
    pub hw: H,
}

impl<'a, H: Hardware, GB: GameboyMode> Peripherals<'a, H, GB> {
    /// Create a new MMU instance.
    pub fn new(mut hw: H, rom: &'a [u8], color: bool, cartridge_ram: &'a mut [u8]) -> Self {
        Self {
            wram: Wram::new(),
            hram: Hram::new(),
            gpu: Gpu::new(),
            mbc: Mbc::new(&mut hw, rom, color, cartridge_ram),
            timer: Timer::new(),
            ic: Ic::new(),
            serial: Serial::new(),
            joypad: Joypad::new(),
            apu: Apu::new(),
            dma: Dma::new(),
            cgb: Cgb::new(color),
            irq: Irq::new(),
            hw,
        }
    }
}

/// The memory management unit (MMU)
///
/// This unit holds a memory byte array which represents address space of the memory.
/// It provides the logic to intercept access from the CPU to the memory byte array,
/// and to modify the memory access behaviour.
pub struct Mmu<'a, 'b, H, GB: GameboyMode> {
    pub peripherals: &'a mut Peripherals<'b, H, GB>,
    pub mixer_stream: &'a mut MixerStream,
}

impl<'a, 'b, H: Hardware, GB: GameboyMode> Mmu<'a, 'b, H, GB> {
    fn io_read(&mut self, addr: u16) -> u8 {
        match addr {
            0xff00 => self.peripherals.joypad.read(&mut self.peripherals.hw),
            0xff01 => self.peripherals.serial.get_data(),
            0xff02 => self.peripherals.serial.get_ctrl(),
            0xff03 => todo!("i/o write: addr={:04x}", addr),
            0xff04..=0xff07 => self.peripherals.timer.on_read(addr),
            0xff08..=0xff0e => todo!("i/o read: addr={:04x}", addr),
            0xff0f => self.peripherals.ic.read_flags(&self.peripherals.irq),
            0xff10 => self.peripherals.apu.read_tone_sweep(),
            0xff11 => self.peripherals.apu.read_tone_wave(0),
            0xff12 => self.peripherals.apu.read_tone_envelop(0),
            0xff13 => self.peripherals.apu.read_tone_freq_low(0),
            0xff14 => self.peripherals.apu.read_tone_freq_high(0),
            0xff16 => self.peripherals.apu.read_tone_wave(1),
            0xff17 => self.peripherals.apu.read_tone_envelop(1),
            0xff18 => self.peripherals.apu.read_tone_freq_low(1),
            0xff19 => self.peripherals.apu.read_tone_freq_high(1),
            0xff1a => self.peripherals.apu.read_wave_enable(),
            0xff1b => self.peripherals.apu.read_wave_len(),
            0xff1c => self.peripherals.apu.read_wave_amp(),
            0xff1d => self.peripherals.apu.read_wave_freq_low(),
            0xff1e => self.peripherals.apu.read_wave_freq_high(),
            0xff20 => self.peripherals.apu.read_noise_len(),
            0xff21 => self.peripherals.apu.read_noise_envelop(),
            0xff22 => self.peripherals.apu.read_noise_poly_counter(),
            0xff23 => self.peripherals.apu.read_noise_select(),
            0xff24 => self.peripherals.apu.read_ctrl(),
            0xff25 => self.peripherals.apu.read_so_mask(),
            0xff26 => self.peripherals.apu.read_enable(),
            0xff30..=0xff3f => self.peripherals.apu.read_wave_buf(addr),
            0xff40 => self.peripherals.gpu.read_ctrl(),
            0xff41 => self.peripherals.gpu.read_status(),
            0xff42 => self.peripherals.gpu.read_scy(),
            0xff43 => self.peripherals.gpu.read_scx(),
            0xff44 => self.peripherals.gpu.read_ly(),
            0xff45 => self.peripherals.gpu.read_lyc(),
            0xff46 => self.peripherals.dma.read(),
            0xff47 => self.peripherals.gpu.read_bg_palette(),
            0xff48 => self.peripherals.gpu.read_obj_palette0(),
            0xff49 => self.peripherals.gpu.read_obj_palette1(),
            0xff4a => self.peripherals.gpu.read_wy(),
            0xff4b => self.peripherals.gpu.read_wx(),
            0xff4d => self.peripherals.cgb.read_speed_switch(),
            0xff4f => self.peripherals.gpu.read_vram_bank_select(),
            0xff51 => self.peripherals.gpu.read_hdma_src_high(),
            0xff52 => self.peripherals.gpu.read_hdma_src_low(),
            0xff53 => self.peripherals.gpu.read_hdma_dst_high(),
            0xff54 => self.peripherals.gpu.read_hdma_dst_low(),
            0xff55 => self.peripherals.gpu.read_hdma_start(),
            0xff56 => todo!("ir"),
            0xff68 => todo!("cgb bg palette index"),
            0xff69 => self.peripherals.gpu.read_bg_color_palette(),
            0xff6a => todo!("cgb bg palette data"),
            0xff6b => self.peripherals.gpu.read_obj_color_palette(),
            0xff70 => self.peripherals.wram.get_bank().get(),
            0x0000..=0xfeff | 0xff80..=0xffff => unreachable!("read non-i/o addr={:04x}", addr),
            _ => {
                warn!("read unknown i/o addr={:04x}", addr);
                0xff
            }
        }
    }

    fn io_write(&mut self, addr: u16, v: u8) {
        match addr {
            0xff00 => self.peripherals.joypad.write(v),
            0xff01 => self.peripherals.serial.set_data(v),
            0xff02 => self
                .peripherals
                .serial
                .set_ctrl(v, &mut self.peripherals.hw),
            0xff03 => todo!("i/o write: addr={:04x}, v={:02x}", addr, v),
            0xff04..=0xff07 => self.peripherals.timer.on_write(addr, v),
            0xff08..=0xff0e => todo!("i/o write: addr={:04x}, v={:02x}", addr, v),
            0xff0f => self
                .peripherals
                .ic
                .write_flags(v, &mut self.peripherals.irq),
            0xff10 => self.peripherals.apu.write_tone_sweep(v),
            0xff11 => self.peripherals.apu.write_tone_wave(0, v),
            0xff12 => self.peripherals.apu.write_tone_envelop(0, v),
            0xff13 => self.peripherals.apu.write_tone_freq_low(0, v),
            0xff14 => self
                .peripherals
                .apu
                .write_tone_freq_high(0, v, self.mixer_stream),
            0xff16 => self.peripherals.apu.write_tone_wave(1, v),
            0xff17 => self.peripherals.apu.write_tone_envelop(1, v),
            0xff18 => self.peripherals.apu.write_tone_freq_low(1, v),
            0xff19 => self
                .peripherals
                .apu
                .write_tone_freq_high(1, v, self.mixer_stream),
            0xff1a => self.peripherals.apu.write_wave_enable(v, self.mixer_stream),
            0xff1b => self.peripherals.apu.write_wave_len(v),
            0xff1c => self.peripherals.apu.write_wave_amp(v),
            0xff1d => self.peripherals.apu.write_wave_freq_low(v),
            0xff1e => self
                .peripherals
                .apu
                .write_wave_freq_high(v, self.mixer_stream),
            0xff20 => self.peripherals.apu.write_noise_len(v),
            0xff21 => self.peripherals.apu.write_noise_envelop(v),
            0xff22 => self.peripherals.apu.write_noise_poly_counter(v),
            0xff23 => self
                .peripherals
                .apu
                .write_noise_select(v, self.mixer_stream),
            0xff24 => self.peripherals.apu.write_ctrl(v, self.mixer_stream),
            0xff25 => self.peripherals.apu.write_so_mask(v, self.mixer_stream),
            0xff26 => self.peripherals.apu.write_enable(v, self.mixer_stream),
            0xff30..=0xff3f => self.peripherals.apu.write_wave_buf(addr, v),
            0xff40 => self
                .peripherals
                .gpu
                .write_ctrl(v, &mut self.peripherals.irq),
            0xff41 => self.peripherals.gpu.write_status(v),
            0xff42 => self.peripherals.gpu.write_scy(v),
            0xff43 => self.peripherals.gpu.write_scx(v),
            0xff44 => self.peripherals.gpu.clear_ly(),
            0xff45 => self.peripherals.gpu.write_lyc(v),
            0xff46 => self.peripherals.dma.start(v),
            0xff47 => self.peripherals.gpu.write_bg_palette(v),
            0xff48 => self.peripherals.gpu.write_obj_palette0(v),
            0xff49 => self.peripherals.gpu.write_obj_palette1(v),
            0xff4a => self.peripherals.gpu.write_wy(v),
            0xff4b => self.peripherals.gpu.write_wx(v),
            0xff4d => self.peripherals.cgb.write_speed_switch(v),
            0xff4f => self.peripherals.gpu.select_vram_bank(v),
            0xff50 => self.peripherals.mbc.disable_boot_rom(v),
            0xff51 => self.peripherals.gpu.write_hdma_src_high(v),
            0xff52 => self.peripherals.gpu.write_hdma_src_low(v),
            0xff53 => self.peripherals.gpu.write_hdma_dst_high(v),
            0xff54 => self.peripherals.gpu.write_hdma_dst_low(v),
            0xff55 => self.peripherals.gpu.write_hdma_start(v),
            0xff56 => todo!("ir"),
            0xff68 => self.peripherals.gpu.select_bg_color_palette(v),
            0xff69 => self.peripherals.gpu.write_bg_color_palette(v),
            0xff6a => self.peripherals.gpu.select_obj_color_palette(v),
            0xff6b => self.peripherals.gpu.write_obj_color_palette(v),
            0xff70 => self.peripherals.wram.select_bank(v),
            0xff7f => {} // off-by-one error in Tetris
            0x0000..=0xfeff | 0xff80..=0xffff => {
                unreachable!("write non-i/o addr={:04x}, v={:02x}", addr, v)
            }
            _ => warn!("write unknown i/o addr={:04x}, v={:02x}", addr, v),
        }
    }

    fn run_dma(&mut self, req: DmaRequest) {
        debug!(
            "DMA Transfer: {:04x} to {:04x} ({:04x} bytes)",
            req.src(),
            req.dst(),
            req.len()
        );
        for i in 0..req.len() {
            let value = self.get8(req.src() + i);
            self.set8(req.dst() + i, value);
        }
    }
}

impl<'a, 'b, T: Hardware, GB: GameboyMode> Sys for Mmu<'a, 'b, T, GB> {
    /// Get the interrupt vector address without clearing the interrupt flag state
    fn peek_int_vec(&mut self) -> Option<u8> {
        self.peripherals.ic.peek(&mut self.peripherals.irq)
    }

    /// Get the interrupt vector address clearing the interrupt flag state
    fn pop_int_vec(&mut self) -> Option<u8> {
        self.peripherals.ic.pop(&mut self.peripherals.irq)
    }

    /// Reads one byte from the given address in the memory.
    fn get8(&mut self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x7fff => self.peripherals.mbc.on_read(addr),
            0x8000..=0x9fff => self.peripherals.gpu.read_vram(addr),
            0xa000..=0xbfff => self.peripherals.mbc.on_read(addr),
            0xc000..=0xfdff => self.peripherals.wram.get8(addr),
            0xfe00..=0xfe9f => self.peripherals.gpu.read_oam(addr),
            0xfea0..=0xfeff => 0, // Unusable range
            0xff00..=0xff7f => self.io_read(addr),
            0xff80..=0xfffe => self.peripherals.hram.get8(addr),
            0xffff..=0xffff => self.peripherals.ic.read_enabled(&self.peripherals.irq),
        }
    }

    /// Writes one byte at the given address in the memory.
    fn set8(&mut self, addr: u16, v: u8) {
        match addr {
            0x0000..=0x7fff => self
                .peripherals
                .mbc
                .on_write(addr, v, &mut self.peripherals.hw),
            0x8000..=0x9fff => self.peripherals.gpu.write_vram(addr, v),
            0xa000..=0xbfff => self
                .peripherals
                .mbc
                .on_write(addr, v, &mut self.peripherals.hw),
            0xc000..=0xfdff => self.peripherals.wram.set8(addr, v),
            0xfe00..=0xfe9f => self.peripherals.gpu.write_oam(addr, v),
            0xfea0..=0xfeff => {} // Unusable range
            0xff00..=0xff7f => self.io_write(addr, v),
            0xff80..=0xfffe => self.peripherals.hram.set8(addr, v),
            0xffff..=0xffff => self
                .peripherals
                .ic
                .write_enabled(v, &mut self.peripherals.irq),
        }
    }

    /// Updates the machine state by the given cycles
    fn step(&mut self, cycles: usize) {
        if let Some(req) = self.peripherals.dma.step(cycles) {
            self.run_dma(req);
        }
        if let Some(req) =
            self.peripherals
                .gpu
                .step(cycles, &mut self.peripherals.irq, &mut self.peripherals.hw)
        {
            self.run_dma(req);
        }
        self.peripherals.apu.step(cycles);
        self.peripherals
            .timer
            .step(cycles, &mut self.peripherals.irq);
        self.peripherals
            .serial
            .step(cycles, &mut self.peripherals.irq, &mut self.peripherals.hw);
        self.peripherals
            .joypad
            .poll(&mut self.peripherals.irq, &mut self.peripherals.hw);
    }
}

/// Behaves as a byte array for unit tests
pub struct Ram {
    ram: [u8; 0x10000],
}

impl Default for Ram {
    fn default() -> Self {
        Self::new()
    }
}

impl Ram {
    /// Create a new ram instance.
    pub fn new() -> Self {
        Self { ram: [0; 0x10000] }
    }

    /// Write a byte array at the beginnig of the memory.
    pub fn write(&mut self, m: &[u8]) {
        for (i, m) in m.iter().enumerate() {
            self.set8(i as u16, *m);
        }
    }
}

impl Sys for Ram {
    fn peek_int_vec(&mut self) -> Option<u8> {
        None
    }

    fn pop_int_vec(&mut self) -> Option<u8> {
        None
    }

    fn get8(&mut self, addr: u16) -> u8 {
        self.ram[addr as usize]
    }

    fn set8(&mut self, addr: u16, v: u8) {
        self.ram[addr as usize] = v;
    }

    fn step(&mut self, _: usize) {}
}
