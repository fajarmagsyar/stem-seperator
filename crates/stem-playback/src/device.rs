use crate::transport::RateMixer;
use cpal::{
    FromSample, SampleFormat, SizedSample,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};
use ringbuf::{
    HeapCons, HeapRb,
    traits::{Consumer, Observer, Producer, Split},
};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc::{self, Sender},
    },
    thread::{self, JoinHandle},
    time::Duration,
};
use stem_core::{
    StemError,
    export::MixSetting,
    types::{SeparationResult, StemId},
};

const FRAME_MASK: u64 = (1_u64 << 48) - 1;
#[derive(Clone, Copy)]
struct Frame {
    samples: [f32; 2],
    position: u64,
    generation: u16,
}
enum Command {
    Seek(u64, u16),
    Mix(Vec<MixSetting>),
    Original(bool),
    Close,
}
struct Shared {
    playing: AtomicBool,
    clock: AtomicU64,
    failed: AtomicBool,
    ended: AtomicBool,
}

pub struct PlaybackEngine {
    stream: Option<cpal::Stream>,
    worker: Option<JoinHandle<()>>,
    sender: Sender<Command>,
    shared: Arc<Shared>,
    settings: Vec<MixSetting>,
    frames: u64,
}

fn stream<T: SizedSample + FromSample<f32>>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    mut consumer: HeapCons<Frame>,
    shared: Arc<Shared>,
) -> Result<cpal::Stream, StemError> {
    let channels = usize::from(config.channels);
    let error_state = shared.clone();
    device
        .build_output_stream(
            config,
            move |output: &mut [T], _| {
                for output in output.chunks_mut(channels) {
                    let mut samples = [0.0; 2];
                    let generation = (shared.clock.load(Ordering::Acquire) >> 48) as u16;
                    while consumer
                        .try_peek()
                        .is_some_and(|frame| frame.generation != generation)
                    {
                        let _ = consumer.try_pop();
                    }
                    if shared.playing.load(Ordering::Acquire) {
                        while let Some(frame) = consumer.try_pop() {
                            if frame.generation != generation {
                                continue;
                            }
                            samples = frame.samples;
                            let old = shared.clock.load(Ordering::Acquire);
                            if (old >> 48) as u16 == generation {
                                let _ = shared.clock.compare_exchange(
                                    old,
                                    (u64::from(generation) << 48) | frame.position,
                                    Ordering::AcqRel,
                                    Ordering::Acquire,
                                );
                            }
                            break;
                        }
                        if consumer.is_empty() && shared.ended.load(Ordering::Acquire) {
                            shared.playing.store(false, Ordering::Release);
                        }
                    }
                    for (channel, target) in output.iter_mut().enumerate() {
                        let sample = if channels == 1 {
                            (samples[0] + samples[1]) * 0.5
                        } else {
                            match channel {
                                0 => samples[0],
                                1 => samples[1],
                                _ => 0.0,
                            }
                        };
                        *target = T::from_sample(sample.clamp(-1.0, 1.0));
                    }
                }
            },
            move |_| {
                error_state.failed.store(true, Ordering::Release);
                error_state.playing.store(false, Ordering::Release);
            },
            None,
        )
        .map_err(|e| StemError::Unavailable(format!("Cannot open audio output: {e}")))
}

impl PlaybackEngine {
    pub fn open(result: &SeparationResult) -> Result<Self, StemError> {
        let device = cpal::default_host()
            .default_output_device()
            .ok_or_else(|| {
                StemError::Unavailable(
                    "No audio output device. Separation and export remain available.".into(),
                )
            })?;
        let supported = device
            .default_output_config()
            .map_err(|e| StemError::Unavailable(e.to_string()))?;
        let format = supported.sample_format();
        let config: cpal::StreamConfig = supported.into();
        let mut mixer = RateMixer::new(result, config.sample_rate.0)?;
        let shared = Arc::new(Shared {
            playing: AtomicBool::new(false),
            clock: AtomicU64::new(0),
            failed: AtomicBool::new(false),
            ended: AtomicBool::new(false),
        });
        let (mut producer, consumer) = HeapRb::<Frame>::new(4096).split();
        let stream = match format {
            SampleFormat::F32 => stream::<f32>(&device, &config, consumer, shared.clone())?,
            SampleFormat::I16 => stream::<i16>(&device, &config, consumer, shared.clone())?,
            SampleFormat::U16 => stream::<u16>(&device, &config, consumer, shared.clone())?,
            _ => {
                return Err(StemError::Unavailable(
                    "Unsupported audio device sample format".into(),
                ));
            }
        };
        stream
            .play()
            .map_err(|e| StemError::Unavailable(format!("Cannot start audio output: {e}")))?;
        let (sender, receiver) = mpsc::channel();
        let worker_shared = shared.clone();
        let worker = thread::spawn(move || {
            let mut pending = Vec::new().into_iter();
            let mut generation = 0_u16;
            'worker: loop {
                for command in receiver.try_iter() {
                    match command {
                        Command::Close => break 'worker,
                        Command::Seek(frame, next) => {
                            if mixer.seek(frame).is_err() {
                                worker_shared.failed.store(true, Ordering::Release);
                                break 'worker;
                            }
                            generation = next;
                            pending = Vec::new().into_iter();
                            worker_shared.ended.store(false, Ordering::Release);
                        }
                        Command::Mix(settings) => {
                            if mixer.mixer.set_mix(&settings).is_err() {
                                worker_shared.failed.store(true, Ordering::Release);
                                break 'worker;
                            }
                        }
                        Command::Original(original) => mixer.original = original,
                    }
                }
                if producer.is_full() {
                    thread::sleep(Duration::from_millis(2));
                    continue;
                }
                if let Some((samples, position)) = pending.next() {
                    let _ = producer.try_push(Frame {
                        samples,
                        position,
                        generation,
                    });
                    continue;
                }
                match mixer.next_block() {
                    Ok(block) if block.is_empty() => {
                        worker_shared.ended.store(true, Ordering::Release);
                        thread::sleep(Duration::from_millis(4));
                    }
                    Ok(block) => pending = block.into_iter(),
                    Err(_) => {
                        worker_shared.failed.store(true, Ordering::Release);
                        worker_shared.playing.store(false, Ordering::Release);
                        break;
                    }
                }
            }
        });
        let settings = result
            .stems
            .iter()
            .map(|s| MixSetting {
                id: s.id.clone(),
                gain: 1.0,
                mute: false,
                solo: false,
            })
            .collect();
        Ok(Self {
            stream: Some(stream),
            worker: Some(worker),
            sender,
            shared,
            settings,
            frames: result.original.spec.frames,
        })
    }
    pub fn play(&self) -> Result<(), StemError> {
        if self.shared.failed.load(Ordering::Acquire) {
            return Err(StemError::Unavailable(
                "Audio device disconnected or preview read failed. Retry the preview device."
                    .into(),
            ));
        }
        if self.position_frames() >= self.frames {
            self.seek(0)?;
        }
        self.shared.playing.store(true, Ordering::Release);
        Ok(())
    }
    pub fn pause(&self) {
        self.shared.playing.store(false, Ordering::Release);
    }
    pub fn is_playing(&self) -> bool {
        self.shared.playing.load(Ordering::Acquire)
    }
    pub fn failed(&self) -> bool {
        self.shared.failed.load(Ordering::Acquire)
    }
    pub fn position_frames(&self) -> u64 {
        self.shared.clock.load(Ordering::Acquire) & FRAME_MASK
    }
    pub fn seek(&self, frame: u64) -> Result<(), StemError> {
        if frame > self.frames || frame > FRAME_MASK {
            return Err(StemError::Unavailable("Seek is outside the track".into()));
        }
        let generation = ((self.shared.clock.load(Ordering::Acquire) >> 48) as u16).wrapping_add(1);
        self.shared.ended.store(false, Ordering::Release);
        self.shared
            .clock
            .store((u64::from(generation) << 48) | frame, Ordering::Release);
        self.sender
            .send(Command::Seek(frame, generation))
            .map_err(|_| StemError::Unavailable("Preview worker stopped".into()))
    }
    pub fn set_mix(
        &mut self,
        id: &StemId,
        gain: f32,
        mute: bool,
        solo: bool,
    ) -> Result<(), StemError> {
        let setting = self
            .settings
            .iter_mut()
            .find(|s| &s.id == id)
            .ok_or_else(|| StemError::Unavailable("Unknown preview stem".into()))?;
        if !gain.is_finite() || !(0.0..=4.0).contains(&gain) {
            return Err(StemError::Unavailable("Invalid gain".into()));
        }
        *setting = MixSetting {
            id: id.clone(),
            gain,
            mute,
            solo,
        };
        self.sender
            .send(Command::Mix(self.settings.clone()))
            .map_err(|_| StemError::Unavailable("Preview worker stopped".into()))
    }
    pub fn set_original(&self, enabled: bool) -> Result<(), StemError> {
        self.sender
            .send(Command::Original(enabled))
            .map_err(|_| StemError::Unavailable("Preview worker stopped".into()))?;
        self.seek(self.position_frames())
    }
    pub fn close(&mut self) {
        self.pause();
        self.stream = None;
        let _ = self.sender.send(Command::Close);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
impl Drop for PlaybackEngine {
    fn drop(&mut self) {
        self.close();
    }
}
