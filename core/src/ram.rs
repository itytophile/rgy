use core::{
    cell::Cell,
    ops::{Index, IndexMut},
};

pub struct Ram<const N: usize> {
    inner: [u8; N],
    min: Cell<usize>,
    max: Cell<usize>,
}

impl<const N: usize> Default for Ram<N> {
    fn default() -> Self {
        Self {
            inner: [0; N],
            min: Cell::new(usize::MAX),
            max: Cell::new(usize::MIN),
        }
    }
}

impl<const N: usize> Index<usize> for Ram<N> {
    type Output = u8;

    fn index(&self, index: usize) -> &Self::Output {
        self.min.set(index.min(self.min.get()));
        self.max.set(index.max(self.max.get()));
        self.inner.index(index)
    }
}

impl<const N: usize> IndexMut<usize> for Ram<N> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        self.min.set(index.min(self.min.get()));
        self.max.set(index.max(self.max.get()));
        self.inner.index_mut(index)
    }
}

impl<const N: usize> Ram<N> {
    pub fn as_slice(&self) -> &[u8] {
        &self.inner
    }
}

impl<const N: usize> Drop for Ram<N> {
    fn drop(&mut self) {
        let min = self.min.get();
        let max = self.max.get();
        log::info!("0x{N:x} min: 0x{min:x} ({min}), max: 0x{max:x} ({max})")
    }
}
