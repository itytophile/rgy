use crate::{
    ic::Irq,
    mmu::{MemRead, MemWrite},
    sound::MixerStream, Hardware,
};

/// The trait which allows to hook I/O access from the CPU.
pub trait IoHandler {
    /// The function is called when the CPU attempts to read the memory-mapped I/O.
    fn on_read(&mut self, addr: u16, mixer_stream: &MixerStream, irq: &Irq) -> MemRead;

    /// The function is called when the CPU attempts to write the memory-mapped I/O.
    fn on_write(
        &mut self,
        addr: u16,
        value: u8,
        mixer_stream: &mut MixerStream,
        irq: &mut Irq,
        hw: &mut impl Hardware
    ) -> MemWrite;
}
