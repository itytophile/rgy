/// The width of the VRAM.
pub const VRAM_WIDTH: usize = 160;

/// The height of the VRAM.
pub const VRAM_HEIGHT: usize = 144;

/// Represents a key of the joypad.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Key {
    /// Cursor right key.
    Right,
    /// Cursor left key.
    Left,
    /// Cursor up key.
    Up,
    /// Cursor down key.
    Down,
    /// A key.
    A,
    /// B key.
    B,
    /// Select key.
    Select,
    /// Start key.
    Start,
}

#[derive(Clone, Copy, Default)]
pub struct JoypadInput {
    pub right: bool,
    pub left: bool,
    pub up: bool,
    pub down: bool,
    pub a: bool,
    pub b: bool,
    pub select: bool,
    pub start: bool,
}

/// Sound wave stream which generates the wave to be played by the sound device.
pub trait Stream: Send + 'static {
    /// The maximum value of the amplitude returned by this stream.
    fn max(&self) -> u16;

    /// The argument takes the sample rate, and the return value indicates the amplitude,
    /// whose max value is determined by [`Stream::max`][].
    fn next(&mut self, rate: u32) -> u16;

    /// Indicate the stream is active.
    fn on(&self) -> bool;
}

/// The interface to abstracts the OS-specific functions.
///
/// The users of this emulator library need to implement this trait,
/// providing OS-specific functions.
pub trait Hardware {
    /// Clock source used by the emulator.
    /// The return value needs to be epoch time in microseconds.
    fn clock(&mut self) -> u64;
}
