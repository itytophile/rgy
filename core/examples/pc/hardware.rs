use log::*;
use minifb::{Scale, Window, WindowOptions};
use rgy::sound::MixerStream;
use std::collections::HashMap;
use std::fs::File;
use std::io::prelude::*;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rgy::{Key, Stream, VRAM_HEIGHT, VRAM_WIDTH};

#[derive(Clone)]
pub struct Hardware {
    rampath: Option<String>,
    pub vram: Arc<Mutex<Vec<u32>>>,
    keystate: Arc<Mutex<HashMap<Key, bool>>>,
    escape: Arc<AtomicBool>,
}

struct Gui {
    window: Window,
    vram: Arc<Mutex<Vec<u32>>>,
    keystate: Arc<Mutex<HashMap<Key, bool>>>,
    escape: Arc<AtomicBool>,
}

impl Gui {
    fn new(
        vram: Arc<Mutex<Vec<u32>>>,
        keystate: Arc<Mutex<HashMap<Key, bool>>>,
        escape: Arc<AtomicBool>,
    ) -> Self {
        let title = if cfg!(feature = "color") {
            "Gay Boy Color"
        } else {
            "Gay Boy"
        };
        let window = match Window::new(
            title,
            VRAM_WIDTH,
            VRAM_HEIGHT,
            WindowOptions {
                resize: false,
                scale: Scale::X4,
                ..WindowOptions::default()
            },
        ) {
            Ok(win) => win,
            Err(err) => {
                panic!("Unable to create window {}", err);
            }
        };

        Self {
            window,
            vram,
            keystate,
            escape,
        }
    }

    fn run(mut self) {
        while !self.escape.load(Ordering::Relaxed) {
            std::thread::sleep(Duration::from_millis(10));
            self.vramupdate();
            self.keyupdate();
        }
    }

    fn vramupdate(&mut self) {
        let vram = self.vram.lock().unwrap().clone();
        self.window.update_with_buffer(&vram).unwrap();
    }

    fn keyupdate(&mut self) {
        if !self.window.is_open() {
            self.escape.store(true, Ordering::Relaxed);
        }

        for (_, v) in self.keystate.lock().unwrap().iter_mut() {
            *v = false;
        }

        if let Some(keys) = self.window.get_keys() {
            for k in keys {
                let gbk = match k {
                    minifb::Key::Right => Key::Right,
                    minifb::Key::Left => Key::Left,
                    minifb::Key::Up => Key::Up,
                    minifb::Key::Down => Key::Down,
                    minifb::Key::Z => Key::A,
                    minifb::Key::X => Key::B,
                    minifb::Key::Space => Key::Select,
                    minifb::Key::Enter => Key::Start,
                    minifb::Key::Escape => {
                        self.escape.store(true, Ordering::Relaxed);
                        return;
                    }
                    _ => continue,
                };

                match self.keystate.lock().unwrap().get_mut(&gbk) {
                    Some(v) => *v = true,
                    None => unreachable!(),
                }
            }
        }
    }
}

impl Hardware {
    pub fn new(
        rampath: Option<String>,
        mixer_stream: Arc<Mutex<MixerStream>>,
        escape: Arc<AtomicBool>,
    ) -> Self {
        let vram = Arc::new(Mutex::new(vec![0; VRAM_WIDTH * VRAM_HEIGHT]));

        let pcm = Pcm;
        pcm.run_forever(mixer_stream);

        let mut keystate = HashMap::new();
        keystate.insert(Key::Right, false);
        keystate.insert(Key::Left, false);
        keystate.insert(Key::Up, false);
        keystate.insert(Key::Down, false);
        keystate.insert(Key::A, false);
        keystate.insert(Key::B, false);
        keystate.insert(Key::Select, false);
        keystate.insert(Key::Start, false);
        let keystate = Arc::new(Mutex::new(keystate));

        Self {
            rampath,
            vram,
            keystate,
            escape,
        }
    }

    pub fn run(self) {
        let bg = Gui::new(
            self.vram.clone(),
            self.keystate.clone(),
            self.escape.clone(),
        );
        bg.run();
    }
}

impl rgy::Hardware for Hardware {
    fn joypad_pressed(&mut self, key: Key) -> bool {
        *self
            .keystate
            .lock()
            .unwrap()
            .get(&key)
            .expect("Logic error in keystate map")
    }

    fn send_byte(&mut self, b: u8) {
        info!("Send byte: {:02x}", b);
    }

    fn recv_byte(&mut self) -> Option<u8> {
        None
    }

    fn clock(&mut self) -> u64 {
        let epoch = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Couldn't get epoch");
        epoch.as_micros() as u64
    }

    fn save_ram(&mut self, ram: &[u8]) {
        match &self.rampath {
            Some(path) => {
                let mut fs = File::create(path).expect("Couldn't open file");
                fs.write_all(ram).expect("Couldn't write file");
            }
            None => {}
        }
    }
}

pub struct Pcm;

impl Pcm {
    pub fn run_forever(self, mixer_stream: Arc<Mutex<MixerStream>>) {
        std::thread::spawn(move || {
            self.run(mixer_stream);
        });
    }

    pub fn run(self, mixer_stream: Arc<Mutex<MixerStream>>) {
        let device = cpal::default_output_device().expect("Failed to get default output device");
        let format = device
            .default_output_format()
            .expect("Failed to get default output format");
        let sample_rate = format.sample_rate.0;
        let event_loop = cpal::EventLoop::new();
        let stream_id = event_loop.build_output_stream(&device, &format).unwrap();
        event_loop.play_stream(stream_id.clone());

        event_loop.run(move |_, data| match data {
            cpal::StreamData::Output {
                buffer: cpal::UnknownTypeOutputBuffer::U16(_buffer),
            } => unimplemented!(),
            cpal::StreamData::Output {
                buffer: cpal::UnknownTypeOutputBuffer::I16(_buffer),
            } => unimplemented!(),
            cpal::StreamData::Output {
                buffer: cpal::UnknownTypeOutputBuffer::F32(mut buffer),
            } => {
                let mut mixer_stream = mixer_stream.lock().unwrap();
                for sample in buffer.chunks_mut(format.channels as usize) {
                    sample.fill(
                        (mixer_stream.next(sample_rate) as u64 * 100 / MixerStream::MAX as u64)
                            as f32
                            / 100.0,
                    );
                }
            }
            _ => (),
        });
    }
}
