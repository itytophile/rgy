use log::debug;

use crate::hardware::JoypadInput;
use crate::ic::{Ints, Irq};

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy,  PartialEq, Eq)]
    struct JoypadFlags: u8 {
        const NOT_DPAD = 1 << 4;
        const NOT_BUTTONS = 1 << 5;
    }
}

impl Default for JoypadFlags {
    fn default() -> Self {
        Self::all()
    }
}

impl Default for Joypad {
    fn default() -> Self {
        Self {
            flags: Default::default(),
            pressed: 0xf,
        }
    }
}

pub struct Joypad {
    flags: JoypadFlags,
    pressed: u8, // what was pressed before the new input
}

impl Joypad {
    pub fn poll(&mut self, irq: &mut Irq, joypad_input: JoypadInput) {
        let pressed = self.check(joypad_input);

        // if pressed has different 0 than self.pressed then the intersection will be lower than self.pressed
        // it means that some 1s have been turned to 0 (1 -> 0 = button pressed)
        irq.request |=
            Ints::from_bits_retain(u8::from(self.pressed & pressed < self.pressed)) & Ints::JOYPAD;

        self.pressed = pressed;
    }

    fn check(&self, joypad_input: JoypadInput) -> u8 {
        if !self.flags.contains(JoypadFlags::NOT_DPAD) {
            // we can shift bits by 4 because JoypadInput's flags have a special order
            self.flags.bits() | (joypad_input.complement().bits() >> 4)
        } else if !self.flags.contains(JoypadFlags::NOT_BUTTONS) {
            self.flags.bits()
                | (joypad_input.complement()
                    & (JoypadInput::A | JoypadInput::B | JoypadInput::SELECT | JoypadInput::START))
                    .bits()
        } else {
            0xff
        }
    }

    pub(crate) fn read(&self, joypad_input: JoypadInput) -> u8 {
        debug!("Joypad read: dir: {:x}", self.flags);
        self.check(joypad_input)
    }

    pub(crate) fn write(&mut self, value: u8) {
        debug!(
            "Joypad write: dir: select={:x}, value={:x}",
            self.flags, value
        );
        self.flags = JoypadFlags::from_bits_truncate(value);
    }
}
