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
    fn on_read(&self, mmu: &Mmu, addr: u16) -> MemRead;

    /// The function is called when the CPU attempts to write to the memory.
    fn on_write(&self, mmu: &Mmu, addr: u16, value: u8) -> MemWrite;
}

/// The handle of a memory handler.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Handle(u64);

/// The memory management unit (MMU)
///
/// This unit holds a memory byte array which represents address space of the memory.
/// It provides the logic to intercept access from the CPU to the memory byte array,
/// and to modify the memory access behaviour.
pub struct Mmu<'a> {
    ram: [u8; 0x10000],
    #[allow(clippy::type_complexity)]
    handlers: ArrayVec<((u16, u16), Handle, &'a dyn MemHandler), 18>,
    hdgen: u64,
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
            hdgen: 0,
        }
    }

    fn next_handle(&mut self) -> Handle {
        let handle = self.hdgen;

        self.hdgen += 1;

        Handle(handle)
    }

    /// Add a new memory handler.
    pub fn add_handler(&mut self, range: (u16, u16), handler: &'a dyn MemHandler) -> Handle {
        let handle = self.next_handle();

        self.handlers.push((range, handle, handler));

        handle
    }

    fn get_handlers_from_address<'b>(
        handlers: &'b [((u16, u16), Handle, &dyn MemHandler)],
        addr: u16,
    ) -> impl Iterator<Item = &'b dyn MemHandler> + 'b {
        handlers.iter().filter_map(move |(range, _, handler)| {
            if (range.0..=range.1).contains(&addr) {
                Some(*handler)
            } else {
                None
            }
        })
    }

    /// Reads one byte from the given address in the memory.
    pub fn get8(&self, addr: u16) -> u8 {
        for handler in Self::get_handlers_from_address(&self.handlers, addr) {
            match handler.on_read(self, addr) {
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
        for handler in Self::get_handlers_from_address(&self.handlers, addr) {
            match handler.on_write(self, addr, v) {
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
        let l = self.get8(addr);
        let h = self.get8(addr + 1);
        (h as u16) << 8 | l as u16
    }

    /// Writes two bytes at the given address in the memory.
    pub fn set16(&mut self, addr: u16, v: u16) {
        self.set8(addr, v as u8);
        self.set8(addr + 1, (v >> 8) as u8);
    }
}
