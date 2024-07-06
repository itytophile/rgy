use arrayvec::ArrayVec;

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

/// The handler to intercept memory access from the CPU.
pub trait MemHandler {
    /// The function is called when the CPU attempts to read from the memory.
    fn on_read(&self, addr: u16) -> MemRead;

    /// The function is called when the CPU attempts to write to the memory.
    fn on_write(&self, addr: u16, value: u8) -> MemWrite;
}

/// The memory management unit (MMU)
///
/// This unit holds a memory byte array which represents address space of the memory.
/// It provides the logic to intercept access from the CPU to the memory byte array,
/// and to modify the memory access behaviour.
pub struct Mmu<'a> {
    ram: [u8; 0x10000],
    #[allow(clippy::type_complexity)]
    handlers: ArrayVec<((u16, u16), &'a dyn MemHandler), 20>,
}

impl Default for Mmu<'_> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a> Mmu<'a> {
    /// Create a new MMU instance.
    pub fn new() -> Mmu<'a> {
        Mmu {
            ram: [0u8; 0x10000],
            handlers: ArrayVec::new(),
        }
    }

    /// Add a new memory handler.
    pub fn add_handler(&mut self, range: (u16, u16), handler: &'a dyn MemHandler) {
        self.handlers.push((range, handler));
    }

    fn get_handler_from_address<'b>(
        handlers: &'b [((u16, u16), &dyn MemHandler)],
        addr: u16,
    ) -> Option<&'b dyn MemHandler> {
        handlers.iter().find_map(move |(range, handler)| {
            if (range.0..=range.1).contains(&addr) {
                Some(*handler)
            } else {
                None
            }
        })
    }

    /// Reads one byte from the given address in the memory.
    pub fn get8(&self, addr: u16) -> u8 {
        if let Some(handler) = Self::get_handler_from_address(&self.handlers, addr) {
            match handler.on_read(addr) {
                MemRead::Replace(alt) => return alt,
                MemRead::PassThrough => {}
            }
        }
        if (0xe000..=0xfdff).contains(&addr) {
            // echo ram
            self.ram[addr as usize - 0x2000]
        } else {
            self.ram[addr as usize]
        }
    }

    /// Writes one byte at the given address in the memory.
    pub fn set8(&mut self, addr: u16, v: u8) {
        if let Some(handler) = Self::get_handler_from_address(&self.handlers, addr) {
            match handler.on_write(addr, v) {
                MemWrite::Replace(alt) => {
                    self.ram[addr as usize] = alt;
                    return;
                }
                MemWrite::PassThrough => {}
                MemWrite::Block => return,
            }
        }

        if (0xe000..=0xfdff).contains(&addr) {
            // echo ram
            self.ram[addr as usize - 0x2000] = v
        } else {
            self.ram[addr as usize] = v
        }
    }

    /// Reads two bytes from the given addresss in the memory.
    pub fn get16(&self, addr: u16) -> u16 {
        u16::from_le_bytes([self.get8(addr), self.get8(addr + 1)])
    }

    /// Writes two bytes at the given address in the memory.
    pub fn set16(&mut self, addr: u16, v: u16) {
        self.set8(addr, v as u8);
        self.set8(addr + 1, (v >> 8) as u8);
    }
}
