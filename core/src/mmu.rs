use crate::{device::IoHandler, ic::Irq, sound::MixerStream, System};

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
    pub system: &'b mut System<'a>,
}

pub struct MmuWithoutMixerStream {
    pub ram: [u8; 0x10000],
}

impl<'a> Default for MmuWithoutMixerStream {
    fn default() -> Self {
        Self { ram: [0; 0x10000] }
    }
}

impl<'a, 'b> Mmu<'a, 'b> {
    fn on_read(&mut self, addr: u16) -> Option<MemRead> {
        match addr {
            0xc000..=0xdfff | 0xff4d | 0xff56 | 0xff70 => {
                Some(self.system.cgb.on_read(addr, self.mixer_stream, self.irq))
            }
            0x0000..=0x7fff | 0xff50 | 0xa000..=0xbfff => {
                Some(self.system.mbc.on_read(addr, self.mixer_stream, self.irq))
            }
            0xff10..=0xff3f => Some(self.system.sound.on_read(addr, self.mixer_stream, self.irq)),
            0xff46 => Some(self.system.dma.on_read(addr, self.mixer_stream, self.irq)),
            0x8000..=0x9fff
            | 0xff40..=0xff45
            | 0xff47..=0xff4b
            | 0xff4f
            | 0xff51..=0xff55
            | 0xff68..=0xff6b => Some(self.system.gpu.on_read(addr, self.mixer_stream, self.irq)),
            0xff0f | 0xffff => Some(self.system.ic.on_read(addr, self.mixer_stream, self.irq)),
            0xff00 => Some(
                self.system
                    .joypad
                    .on_read(addr, self.mixer_stream, self.irq),
            ),
            0xff04..=0xff07 => Some(self.system.timer.on_read(addr, self.mixer_stream, self.irq)),
            0xff01..=0xff02 => Some(
                self.system
                    .serial
                    .on_read(addr, self.mixer_stream, self.irq),
            ),
            _ => None,
        }
    }

    fn on_write(&mut self, addr: u16, v: u8) -> Option<MemWrite> {
        match addr {
            0xc000..=0xdfff | 0xff4d | 0xff56 | 0xff70 => Some(self.system.cgb.on_write(
                addr,
                v,
                self.mixer_stream,
                self.irq,
            )),
            0x0000..=0x7fff | 0xff50 | 0xa000..=0xbfff => Some(self.system.mbc.on_write(
                addr,
                v,
                self.mixer_stream,
                self.irq,
            )),
            0xff10..=0xff3f => Some(self.system.sound.on_write(
                addr,
                v,
                self.mixer_stream,
                self.irq,
            )),
            0xff46 => Some(
                self.system
                    .dma
                    .on_write(addr, v, self.mixer_stream, self.irq),
            ),
            0x8000..=0x9fff
            | 0xff40..=0xff45
            | 0xff47..=0xff4b
            | 0xff4f
            | 0xff51..=0xff55
            | 0xff68..=0xff6b => Some(self.system.gpu.on_write(
                addr,
                v,
                self.mixer_stream,
                self.irq,
            )),
            0xff0f | 0xffff => Some(
                self.system
                    .ic
                    .on_write(addr, v, self.mixer_stream, self.irq),
            ),
            0xff00 => Some(
                self.system
                    .joypad
                    .on_write(addr, v, self.mixer_stream, self.irq),
            ),
            0xff04..=0xff07 => Some(self.system.timer.on_write(
                addr,
                v,
                self.mixer_stream,
                self.irq,
            )),
            0xff01..=0xff02 => Some(self.system.serial.on_write(
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
}
