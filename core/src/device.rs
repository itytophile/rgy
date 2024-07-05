use core::cell::{Ref, RefCell, RefMut};

use crate::mmu::{MemHandler, MemRead, MemWrite, Mmu};

/// The wrapper type for I/O handlers to register to MMU.
pub struct Device<'a, T>(&'a RefCell<T>, bool);

impl<'a, T> Clone for Device<'a, T> {
    fn clone(&self) -> Self {
        Self(self.0, self.1)
    }
}

impl<'a, T> Device<'a, T> {
    /// Create a new device.
    pub fn new(inner: &'a RefCell<T>) -> Self {
        Self::inner(inner, false)
    }

    /// Create a new mediater device.
    pub fn mediate(inner: &'a RefCell<T>) -> Self {
        Self::inner(inner, true)
    }

    fn inner(inner: &'a RefCell<T>, debug: bool) -> Self {
        Self(inner, debug)
    }

    /// Immutably borrow the underlying I/O handler.
    pub fn borrow(&self) -> Ref<'a, T> {
        self.0.borrow()
    }

    /// Mutabully borrow the underlying I/O handler.
    pub fn borrow_mut(&self) -> RefMut<'a, T> {
        self.0.borrow_mut()
    }
}

impl<'a, T: IoHandler> Device<'a, T> {
    /// Return the memory-mapped I/O handler of the device.
    pub fn handler(&self) -> IoMemHandler<'a, T> {
        IoMemHandler(self.0, self.1)
    }
}

/// The trait which allows to hook I/O access from the CPU.
pub trait IoHandler {
    /// The function is called when the CPU attempts to read the memory-mapped I/O.
    fn on_read(&mut self, mmu: &Mmu, addr: u16) -> MemRead;

    /// The function is called when the CPU attempts to write the memory-mapped I/O.
    fn on_write(&mut self, mmu: &Mmu, addr: u16, value: u8) -> MemWrite;
}

/// The handler to intercept memory-mapped I/O.
pub struct IoMemHandler<'a, T>(&'a RefCell<T>, bool);

impl<'a, T: IoHandler> MemHandler for IoMemHandler<'a, T>
where
    T: IoHandler,
{
    fn on_read(&self, mmu: &Mmu, addr: u16) -> MemRead {
        // Don't hook if it's already hooked
        match self.0.try_borrow_mut() {
            Ok(mut inner) => inner.on_read(mmu, addr),
            Err(e) => {
                if self.1 {
                    // In mediator mode, allow to recursive read
                    MemRead::PassThrough
                } else {
                    panic!("Recursive read from {:04x}: {}", addr, e)
                }
            }
        }
    }

    fn on_write(&self, mmu: &Mmu, addr: u16, value: u8) -> MemWrite {
        // Don't hook if it's already hooked
        match self.0.try_borrow_mut() {
            Ok(mut inner) => inner.on_write(mmu, addr, value),
            Err(e) => {
                if self.1 {
                    // In mediator mode, allow to recursive write
                    MemWrite::PassThrough
                } else {
                    panic!("Recursive write to {:04x}: {}", addr, e)
                }
            }
        }
    }
}
