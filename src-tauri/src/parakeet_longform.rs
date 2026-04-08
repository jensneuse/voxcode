use transcribe_rs::onnx::parakeet::ParakeetModel;
use transcribe_rs::transcriber::{EnergyAdaptiveChunked, EnergyAdaptiveConfig, Transcriber};
use transcribe_rs::{TranscribeOptions, TranscriptionResult};

use crate::error::{Error, Result};

const ENERGY_TARGET_CHUNK_SECS: f32 = 240.0;
const ENERGY_SEARCH_WINDOW_SECS: f32 = 5.0;
const ENERGY_PADDING_SECS: f32 = 0.2;

pub fn transcribe_energy_adaptive(
    model: &mut ParakeetModel,
    audio: &[f32],
) -> Result<TranscriptionResult> {
    let config = EnergyAdaptiveConfig {
        target_chunk_secs: ENERGY_TARGET_CHUNK_SECS,
        search_window_secs: ENERGY_SEARCH_WINDOW_SECS,
        padding_secs: ENERGY_PADDING_SECS,
        min_chunk_secs: 0.0,
        frame_size: 480,
        merge_separator: " ".into(),
    };
    let mut chunker = EnergyAdaptiveChunked::new(config, TranscribeOptions::default());

    chunker
        .transcribe(model, audio)
        .map_err(|e| Error::Parakeet(format!("Parakeet chunked transcription failed: {e}")))
}
