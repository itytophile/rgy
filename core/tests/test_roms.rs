use std::{
    io::Write,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use rgy::{apu::mixer::MixerStream, gpu::DmgColor, mmu::DmgMode, VRAM_HEIGHT, VRAM_WIDTH};

#[derive(Clone)]
enum Expected {
    Serial(&'static str),
    Display(Vec<DmgColor>),
}

impl Expected {
    fn from_file(path: &str) -> Self {
        let display: Vec<DmgColor> = std::fs::read_to_string(path)
            .unwrap()
            .chars()
            .filter_map(|c| match c {
                '.' => Some(DmgColor::White),
                '#' => Some(DmgColor::Black),
                _ => None,
            })
            .collect();
        assert_eq!(VRAM_HEIGHT * VRAM_WIDTH, display.len());
        Self::Display(display)
    }
}

struct TestHardware;

impl rgy::Hardware for TestHardware {
    fn clock(&mut self) -> u64 {
        let epoch = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
        epoch.as_micros() as u64
    }

    fn save_ram(&mut self, _: &[u8]) {}
}

fn test_rom(expected: Expected, path: &str) {
    let rom = std::fs::read(path).unwrap();
    let mut cartridge_ram = [0; 0x8000];
    let mut sys =
        rgy::System::<_, DmgMode>::new(Default::default(), &rom, TestHardware, &mut cartridge_ram);
    const TIMEOUT: Duration = Duration::from_secs(60);
    let now = Instant::now();
    let mut mixer_stream = MixerStream::new();
    let mut display = [DmgColor::White; VRAM_HEIGHT * VRAM_WIDTH];
    let mut index = 0;
    loop {
        let poll_data = sys.poll(&mut mixer_stream, Default::default(), &mut None);
        if now.elapsed() >= TIMEOUT {
            panic!("timeout")
        }
        match &expected {
            Expected::Serial(expected) => {
                if poll_data.serial_sent_bytes.is_empty() {
                    continue;
                }
                print!(
                    "{}",
                    std::str::from_utf8(poll_data.serial_sent_bytes).unwrap()
                );
                std::io::stdout().flush().unwrap();
                for &b in poll_data.serial_sent_bytes {
                    assert_eq!(
                        expected.as_bytes()[index] as char,
                        b as char,
                        "error at index {}, expected: {:?}",
                        index,
                        &expected[0..=index]
                    );
                    index += 1;
                    if index == expected.len() {
                        return;
                    }
                }
            }
            Expected::Display(expected) => {
                let Some((ly, buf)) = poll_data.line_to_draw else {
                    continue;
                };
                for (a, b) in display[usize::from(ly) * VRAM_WIDTH..].iter_mut().zip(buf) {
                    *a = *b;
                }

                if usize::from(ly) == VRAM_HEIGHT - 1 && display.as_slice() == expected.as_slice() {
                    return;
                }
            }
        }

        // // print display to console
        // if usize::from(ly) == VRAM_HEIGHT - 1 {
        //     println!();
        //     for (index, color) in display.iter().enumerate() {
        //         if *color == DmgColor::White {
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
