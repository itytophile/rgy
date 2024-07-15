use log::*;

/// Sharable handle for I/O devices to request/cancel interrupts
#[derive(Default)]
pub struct Irq {
    enable: Ints,
    request: Ints,
}

impl Irq {
    /// Request/cacnel vblank interrupt
    pub fn vblank(&mut self, v: bool) {
        self.request.set(Ints::VBLANK, v);
    }

    /// Request/cancel LCD interrupt
    pub fn lcd(&mut self, v: bool) {
        self.request.set(Ints::LCD, v);
    }

    /// Request/cancel timer interrupt
    pub fn timer(&mut self, v: bool) {
        self.request.set(Ints::TIMER, v);
    }

    /// Request/cancel serial interrupt
    pub fn serial(&mut self, v: bool) {
        self.request.set(Ints::SERIAL, v);
    }

    /// Request/cancel joypad interrupt
    pub fn joypad(&mut self, v: bool) {
        self.request.set(Ints::JOYPAD, v);
    }
}

// The `bitflags!` macro generates `struct`s that manage a set of flags.
bitflags::bitflags! {
    /// Represents a set of flags.
    #[derive(Debug, Clone, Default, Copy,  PartialEq, Eq)]
    struct Ints: u8 {
        const VBLANK = 1;
        const LCD = 1 << 1;
        const TIMER = 1 << 2;
        const SERIAL = 1 << 3;
        const JOYPAD = 1 << 4;
    }
}

/// Get the interrupt vector address without clearing the interrupt flag state
pub fn peek(irq: &mut Irq) -> Option<u8> {
    check(false, irq)
}

/// Get the interrupt vector address clearing the interrupt flag state
pub fn pop(irq: &mut Irq) -> Option<u8> {
    check(true, irq)
}

fn check(consume: bool, irq: &mut Irq) -> Option<u8> {
    let e = &irq.enable;
    let r = &mut irq.request;

    for (flag, value) in [
        (Ints::VBLANK, 0x40),
        (Ints::LCD, 0x48),
        (Ints::TIMER, 0x50),
        (Ints::SERIAL, 0x58),
        (Ints::JOYPAD, 0x60),
    ] {
        if e.contains(flag) && r.contains(flag) {
            r.set(flag, !consume);
            return Some(value);
        }
    }
    None
}

/// Read IE register (0xffff)
pub fn read_enabled(irq: &Irq) -> u8 {
    let v = irq.enable.bits();
    info!("Read interrupt enable: {:02x}", v);
    v
}

/// Write IF register (0xff0f)
pub fn read_flags(irq: &Irq) -> u8 {
    let v = irq.request.bits();
    info!("Read interrupt: {:02x}", v);
    v | 0xe0
}

/// Write IE register (0xffff)
pub fn write_enabled(value: u8, irq: &mut Irq) {
    info!("Write interrupt enable: {:02x}", value);
    irq.enable = Ints::from_bits_retain(value);
}

/// Write IF register (0xff0f)
pub fn write_flags(value: u8, irq: &mut Irq) {
    info!("Write interrupt: {:02x}", value);
    irq.request = Ints::from_bits_retain(value);
}
