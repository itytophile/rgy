use std::{
    io::Write,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use rgy::{apu::mixer::MixerStream, mmu::DmgMode, VRAM_HEIGHT, VRAM_WIDTH};

#[derive(Clone)]
enum Expected {
    Serial(&'static str),
    Display(Vec<u32>),
}

impl Expected {
    fn from_file(path: &str) -> Self {
        let display: Vec<u32> = std::fs::read_to_string(path)
            .unwrap()
            .chars()
            .filter_map(|c| match c {
                '.' => Some(0xdddddd),
                '#' => Some(0x555555),
                _ => None,
            })
            .collect();
        assert_eq!(VRAM_HEIGHT * VRAM_WIDTH, display.len());
        Self::Display(display)
    }
}

struct TestHardware {
    expected: Expected,
    index: usize,
    is_done: bool,
}

impl TestHardware {
    fn new(expected: Expected) -> Self {
        Self {
            expected,
            index: 0,
            is_done: false,
        }
    }
}

impl rgy::Hardware for TestHardware {
    fn joypad_pressed(&mut self, _: rgy::Key) -> bool {
        false
    }

    fn clock(&mut self) -> u64 {
        let epoch = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
        epoch.as_micros() as u64
    }

    fn send_byte(&mut self, b: u8) {
        let Expected::Serial(expected) = self.expected else {
            return;
        };
        if self.is_done {
            return;
        }
        print!("{}", b as char);
        std::io::stdout().flush().unwrap();
        assert_eq!(
            expected.as_bytes()[self.index] as char,
            b as char,
            "error at index {}, expected: {:?}",
            self.index,
            &expected[0..=self.index]
        );
        self.index += 1;
        if self.index == expected.len() {
            self.is_done = true;
        }
    }

    fn recv_byte(&mut self) -> Option<u8> {
        None
    }

    fn save_ram(&mut self, _: &[u8]) {}

    fn sched(&mut self) -> bool {
        !self.is_done
    }
}

fn test_rom(expected: Expected, path: &str) {
    let rom = std::fs::read(path).unwrap();
    let hw = TestHardware::new(expected.clone());
    let mut cartridge_ram = [0; 0x8000];
    let mut sys = rgy::System::<_, DmgMode>::new(Default::default(), &rom, hw, &mut cartridge_ram);
    const TIMEOUT: Duration = Duration::from_secs(60);
    let now = Instant::now();
    let mut mixer_stream = MixerStream::new();
    let mut display = [0; VRAM_HEIGHT * VRAM_WIDTH];
    while let Some(poll_data) = sys.poll(&mut mixer_stream) {
        if now.elapsed() >= TIMEOUT {
            panic!("timeout")
        }
        let Expected::Display(expected) = &expected else {
            continue;
        };
        let Some((ly, buf)) = poll_data.line_to_draw else {
            continue;
        };
        display[usize::from(ly) * VRAM_WIDTH..(usize::from(ly) + 1) * VRAM_WIDTH]
            .copy_from_slice(buf);

        if usize::from(ly) == VRAM_HEIGHT - 1 && display.as_slice() == expected.as_slice() {
            return;
        }

        // // print display to console
        // if ly == VRAM_HEIGHT - 1 {
        //     println!();
        //     for (index, color) in self.display.iter().enumerate() {
        //         if *color == 0xdddddd {
        //             print!(".")
        //         } else {
        //             print!("#")
        //         }
        //         if index % VRAM_WIDTH == VRAM_WIDTH - 1 {
        //             println!();
        //         }
        //     }
        // }
    }
}

#[test]
fn cpu_instrs() {
    const EXPECTED: &str = "cpu_instrs\n\n01:ok  02:ok  03:ok  04:ok  05:ok  06:ok  07:ok  08:ok  09:ok  10:ok  11:ok  \n\nPassed all tests";
    test_rom(
        Expected::Serial(EXPECTED),
        "../roms/cpu_instrs/cpu_instrs.gb",
    );
}

#[test]
fn instr_timing() {
    const EXPECTED: &str = "instr_timing\n\n\nPassed";
    test_rom(
        Expected::Serial(EXPECTED),
        "../roms/instr_timing/instr_timing.gb",
    );
}

#[test]
fn mem_timing() {
    const EXPECTED: &str = "mem_timing\n\n01:ok  02:ok  03:ok  \n\nPassed all tests";
    test_rom(
        Expected::Serial(EXPECTED),
        "../roms/mem_timing/mem_timing.gb",
    );
}

#[test]
fn mem_timing2() {
    test_rom(
        Expected::from_file("tests/mem_timing2.txt"),
        "../roms/mem_timing-2/mem_timing.gb",
    );
}

#[test]
fn halt_bug() {
    test_rom(
        Expected::from_file("tests/halt_bug.txt"),
        "../roms/halt_bug.gb",
    );
}
