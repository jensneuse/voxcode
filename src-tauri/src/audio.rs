use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use rubato::{
    Resampler, SincFixedIn, SincInterpolationParameters, SincInterpolationType, WindowFunction,
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

const TARGET_SAMPLE_RATE: u32 = 16_000;
const RESAMPLER_CHUNK_SIZE: usize = 128;

pub struct AudioInputConfig {
    pub target_sample_rate: u32,
    pub resampler_chunk_size: usize,
}

impl Default for AudioInputConfig {
    fn default() -> Self {
        Self {
            target_sample_rate: TARGET_SAMPLE_RATE,
            resampler_chunk_size: RESAMPLER_CHUNK_SIZE,
        }
    }
}

pub struct AudioInputWorker {
    config: AudioInputConfig,
    stop_signal: Arc<AtomicBool>,
    amplitude_tx: Option<Sender<f32>>,
    accum_tx: Option<Sender<Vec<f32>>>,
}

impl AudioInputWorker {
    pub fn new(stop_signal: Arc<AtomicBool>) -> Self {
        Self {
            config: AudioInputConfig::default(),
            stop_signal,
            amplitude_tx: None,
            accum_tx: None,
        }
    }

    pub fn set_amplitude_sender(&mut self, sender: Sender<f32>) {
        self.amplitude_tx = Some(sender);
    }

    pub fn set_accumulator_sender(&mut self, sender: Sender<Vec<f32>>) {
        self.accum_tx = Some(sender);
    }

    pub fn start(self) -> JoinHandle<()> {
        thread::spawn(move || {
            if let Err(err) = self.run_capture() {
                log::error!("Audio capture error: {err}");
            }
        })
    }

    fn run_capture(self) -> Result<(), Box<dyn std::error::Error>> {
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or("no input device available")?;

        log::info!("Audio device: {}", device.name().unwrap_or_default());

        let config = device.default_input_config()?;
        let device_rate = config.sample_rate().0;
        let channels = config.channels() as usize;

        log::info!("Device sample rate: {device_rate} Hz, channels: {channels}");

        let err_fn = |err| log::error!("cpal stream error: {err}");

        let (stream, mono_acc, target_sample_rate) = match config.sample_format() {
            cpal::SampleFormat::F32 => {
                let amplitude_tx = self.amplitude_tx.clone();
                let stop_signal = self.stop_signal.clone();
                let target_sample_rate = self.config.target_sample_rate;
                let resampler_chunk_size = self.config.resampler_chunk_size;
                let mono_acc = Arc::new(Mutex::new(Vec::with_capacity(resampler_chunk_size * 2)));
                let mono_acc_for_stream = mono_acc.clone();
                let accum_tx_for_stream = self.accum_tx.clone();

                let stream = device.build_input_stream(
                    &config.into(),
                    move |data: &[f32], _| {
                        if stop_signal.load(Ordering::Relaxed) {
                            return;
                        }

                        let mut mono_acc = mono_acc_for_stream.lock().unwrap();
                        mono_acc.extend(downmix_to_mono(data, channels));
                        send_amplitude(&amplitude_tx, &mono_acc);
                        forward_resampled_audio(
                            &mut mono_acc,
                            device_rate,
                            target_sample_rate,
                            resampler_chunk_size,
                            &accum_tx_for_stream,
                        );
                    },
                    err_fn,
                    None,
                )?;
                (stream, mono_acc, target_sample_rate)
            }
            cpal::SampleFormat::I16 => {
                let amplitude_tx = self.amplitude_tx.clone();
                let stop_signal = self.stop_signal.clone();
                let target_sample_rate = self.config.target_sample_rate;
                let resampler_chunk_size = self.config.resampler_chunk_size;
                let mono_acc = Arc::new(Mutex::new(Vec::with_capacity(resampler_chunk_size * 2)));
                let mono_acc_for_stream = mono_acc.clone();
                let accum_tx_for_stream = self.accum_tx.clone();

                let stream = device.build_input_stream(
                    &config.into(),
                    move |data: &[i16], _| {
                        if stop_signal.load(Ordering::Relaxed) {
                            return;
                        }

                        let normalized: Vec<f32> =
                            data.iter().map(|sample| *sample as f32 / i16::MAX as f32).collect();
                        let mut mono_acc = mono_acc_for_stream.lock().unwrap();
                        mono_acc.extend(downmix_to_mono(&normalized, channels));
                        send_amplitude(&amplitude_tx, &mono_acc);
                        forward_resampled_audio(
                            &mut mono_acc,
                            device_rate,
                            target_sample_rate,
                            resampler_chunk_size,
                            &accum_tx_for_stream,
                        );
                    },
                    err_fn,
                    None,
                )?;
                (stream, mono_acc, target_sample_rate)
            }
            cpal::SampleFormat::U16 => {
                let amplitude_tx = self.amplitude_tx.clone();
                let stop_signal = self.stop_signal.clone();
                let target_sample_rate = self.config.target_sample_rate;
                let resampler_chunk_size = self.config.resampler_chunk_size;
                let mono_acc = Arc::new(Mutex::new(Vec::with_capacity(resampler_chunk_size * 2)));
                let mono_acc_for_stream = mono_acc.clone();
                let accum_tx_for_stream = self.accum_tx.clone();

                let stream = device.build_input_stream(
                    &config.into(),
                    move |data: &[u16], _| {
                        if stop_signal.load(Ordering::Relaxed) {
                            return;
                        }

                        let normalized: Vec<f32> = data
                            .iter()
                            .map(|sample| (*sample as f32 / u16::MAX as f32) * 2.0 - 1.0)
                            .collect();
                        let mut mono_acc = mono_acc_for_stream.lock().unwrap();
                        mono_acc.extend(downmix_to_mono(&normalized, channels));
                        send_amplitude(&amplitude_tx, &mono_acc);
                        forward_resampled_audio(
                            &mut mono_acc,
                            device_rate,
                            target_sample_rate,
                            resampler_chunk_size,
                            &accum_tx_for_stream,
                        );
                    },
                    err_fn,
                    None,
                )?;
                (stream, mono_acc, target_sample_rate)
            }
            fmt => return Err(format!("unsupported sample format: {fmt}").into()),
        };

        stream.play()?;
        log::info!("Audio capture started");

        while !self.stop_signal.load(Ordering::Relaxed) {
            thread::sleep(std::time::Duration::from_millis(50));
        }

        log::info!("Audio capture stopping");
        let _ = stream.pause();
        thread::sleep(std::time::Duration::from_millis(250));
        let accum_tx_for_flush = self.accum_tx.clone();
        flush_residual_audio(
            &mut mono_acc.lock().unwrap(),
            device_rate,
            target_sample_rate,
            &accum_tx_for_flush,
        );
        drop(stream);

        Ok(())
    }
}

pub fn compute_rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }

    let sum_sq: f32 = samples.iter().map(|sample| sample * sample).sum();
    (sum_sq / samples.len() as f32).sqrt()
}

pub fn downmix_to_mono(input: &[f32], channels: usize) -> Vec<f32> {
    match channels {
        0 => Vec::new(),
        1 => input.to_vec(),
        _ => input
            .chunks(channels)
            .filter_map(|frame| frame.first().copied())
            .collect(),
    }
}

pub fn resample_to_mono_16khz(
    input: &[f32],
    input_sample_rate: u32,
) -> Result<Vec<f32>, Box<dyn std::error::Error>> {
    if input.is_empty() {
        return Ok(Vec::new());
    }
    if input_sample_rate == TARGET_SAMPLE_RATE {
        return Ok(input.to_vec());
    }

    let expected_len =
        ((input.len() as u64 * TARGET_SAMPLE_RATE as u64) / input_sample_rate as u64) as usize;
    if expected_len == 0 {
        return Ok(Vec::new());
    }

    let step = input_sample_rate as f64 / TARGET_SAMPLE_RATE as f64;
    let mut output = Vec::with_capacity(expected_len);

    for index in 0..expected_len {
        let source_index = index as f64 * step;
        let left_index = source_index.floor() as usize;
        let right_index = (left_index + 1).min(input.len() - 1);
        let mix = (source_index - left_index as f64) as f32;
        let left = input[left_index];
        let right = input[right_index];
        output.push(left + (right - left) * mix);
    }

    Ok(output)
}

fn build_resampler(
    ratio: f64,
    chunk_size: usize,
) -> Result<SincFixedIn<f32>, Box<dyn std::error::Error>> {
    let params = SincInterpolationParameters {
        sinc_len: 256,
        f_cutoff: 0.95,
        interpolation: SincInterpolationType::Linear,
        oversampling_factor: 256,
        window: WindowFunction::BlackmanHarris2,
    };

    Ok(SincFixedIn::<f32>::new(ratio, 2.0, params, chunk_size, 1)?)
}

fn send_amplitude(amplitude_tx: &Option<Sender<f32>>, samples: &[f32]) {
    if let Some(sender) = amplitude_tx {
        let _ = sender.send(compute_rms(samples));
    }
}

fn forward_resampled_audio(
    mono_acc: &mut Vec<f32>,
    device_rate: u32,
    target_sample_rate: u32,
    resampler_chunk_size: usize,
    accum_tx: &Option<Sender<Vec<f32>>>,
) {
    if mono_acc.len() < resampler_chunk_size {
        return;
    }

    let process_len = mono_acc.len() - (mono_acc.len() % resampler_chunk_size);
    let drained: Vec<f32> = mono_acc.drain(..process_len).collect();

    match resample_to_rate_in_chunks(&drained, device_rate, target_sample_rate, resampler_chunk_size)
    {
        Ok(output) if !output.is_empty() => {
            if let Some(sender) = accum_tx {
                let _ = sender.send(output);
            }
        }
        Ok(_) => {}
        Err(err) => log::error!("Resample error: {err}"),
    }
}

fn flush_residual_audio(
    mono_acc: &mut Vec<f32>,
    device_rate: u32,
    target_sample_rate: u32,
    accum_tx: &Option<Sender<Vec<f32>>>,
) {
    if mono_acc.is_empty() {
        return;
    }

    let drained = std::mem::take(mono_acc);
        if target_sample_rate != TARGET_SAMPLE_RATE {
            log::warn!(
                "Residual flush assumes {} Hz target audio, got {} Hz",
                TARGET_SAMPLE_RATE,
                target_sample_rate
            );
        }
        match resample_to_mono_16khz(&drained, device_rate) {
        Ok(output) if !output.is_empty() => {
            if let Some(sender) = accum_tx {
                let _ = sender.send(output);
            }
        }
        Ok(_) => {}
        Err(err) => log::error!("Residual resample error: {err}"),
    }
}

fn resample_to_rate_in_chunks(
    input: &[f32],
    input_sample_rate: u32,
    output_sample_rate: u32,
    chunk_size: usize,
) -> Result<Vec<f32>, Box<dyn std::error::Error>> {
    if input.is_empty() {
        return Ok(Vec::new());
    }
    if input_sample_rate == output_sample_rate {
        return Ok(input.to_vec());
    }

    let ratio = output_sample_rate as f64 / input_sample_rate as f64;
    let mut resampler = build_resampler(ratio, chunk_size)?;
    let input_size = resampler.input_frames_next();
    let mut mono_acc = input.to_vec();
    let mut output = Vec::new();

    while mono_acc.len() >= input_size {
        let chunk: Vec<f32> = mono_acc.drain(..input_size).collect();
        let resampled = resampler.process(&[chunk], None)?;
        output.extend_from_slice(&resampled[0]);
    }

    Ok(output)
}

