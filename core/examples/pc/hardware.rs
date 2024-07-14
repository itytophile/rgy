use minifb::{Scale, Window, WindowOptions};
use rgy::apu::mixer::MixerStream;
use rgy::hardware::JoypadInput;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rgy::{Stream, VRAM_HEIGHT, VRAM_WIDTH};

#[derive(Clone)]
pub struct Hardware;

struct Gui {
    window: Window,
    vram: Arc<Mutex<Vec<u32>>>,
    keystate: Arc<Mutex<JoypadInput>>,
    escape: Arc<AtomicBool>,
}

impl Gui {
    fn new(
        vram: Arc<Mutex<Vec<u32>>>,
        joypad_input: Arc<Mutex<JoypadInput>>,
        escape: Arc<AtomicBool>,
        color: bool,
    ) -> Self {
        let title = if color { "Gay Boy Color" } else { "Gay Boy" };
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
            keystate: joypad_input,
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

        if let Some(keys) = self.window.get_keys() {
            let mut input = self.keystate.lock().unwrap();

            *input = JoypadInput::default();

            for k in keys {
                match k {
                    minifb::Key::Right => input.right = true,
                    minifb::Key::Left => input.left = true,
                    minifb::Key::Up => input.up = true,
                    minifb::Key::Down => input.down = true,
                    minifb::Key::Z => input.a = true,
                    minifb::Key::X => input.b = true,
                    minifb::Key::Space => input.select = true,
                    minifb::Key::Enter => input.start = true,
                    minifb::Key::Escape => {
                        self.escape.store(true, Ordering::Relaxed);
                        return;
                    }
                    _ => continue,
                };
            }
        }
    }
}

pub fn run(
    color: bool,
    mixer_stream: Arc<Mutex<MixerStream>>,
    vram: Arc<Mutex<Vec<u32>>>,
    joypad_input: Arc<Mutex<JoypadInput>>,
    escape: Arc<AtomicBool>,
) {
    let pcm = Pcm;
    pcm.run_forever(mixer_stream);

    let bg = Gui::new(vram, joypad_input, escape, color);
    bg.run()
}

impl rgy::Clock for Hardware {
    fn clock(&self) -> u64 {
        let epoch = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Couldn't get epoch");
        epoch.as_micros() as u64
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
                let mut s = mixer_stream.lock().unwrap();
                for sample in buffer.chunks_mut(format.channels as usize) {
                    sample.fill((s.next(sample_rate) as u64 * 100 / s.max() as u64) as f32 / 100.0);
                }
            }
            _ => (),
        });
    }
}
