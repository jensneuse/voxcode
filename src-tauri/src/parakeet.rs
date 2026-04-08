use crate::error::{Error, Result};
use crate::manifest_dir;
use crate::parakeet_longform::transcribe_energy_adaptive;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use transcribe_rs::onnx::parakeet::{ParakeetModel, ParakeetParams};
pub use transcribe_rs::onnx::Quantization;

const MODEL_DIR_NAME: &str = "models/parakeet-tdt-0.6b-v2";

static PARAKEET_MODEL: OnceLock<Mutex<ParakeetModel>> = OnceLock::new();

const REQUIRED_FILES: &[&str] = &[
    "encoder-model.onnx",
    "encoder-model.onnx.data",
    "decoder_joint-model.onnx",
    "nemo128.onnx",
    "vocab.txt",
];

fn has_required_files(path: &Path) -> bool {
    REQUIRED_FILES.iter().all(|name| path.join(name).is_file())
}

pub fn find_model_dir(resource_dir: Option<&Path>) -> Result<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(resource_dir) = resource_dir {
        candidates.push(resource_dir.join(MODEL_DIR_NAME));
    }
    candidates.push(manifest_dir().join(format!("resources/{MODEL_DIR_NAME}")));
    candidates.push(manifest_dir().join(format!("../{MODEL_DIR_NAME}")));

    candidates
        .into_iter()
        .find(|c| c.exists() && has_required_files(c))
        .ok_or_else(|| {
            Error::Parakeet(
                "Parakeet TDT model not found in bundled, development, or repo-root locations"
                    .into(),
            )
        })
}

/// Load the Parakeet model into the global singleton.
/// Call `ort_init::init()` before this — ORT must be initialized first.
pub fn init() -> Result<()> {
    let model_dir = find_model_dir(None)?;
    init_from_dir(&model_dir)
}

/// Load from a specific directory (used by startup with resource_dir).
pub fn init_from_dir(model_dir: &Path) -> Result<()> {
    init_from_dir_with_quantization(model_dir, Quantization::default())
}

/// Load from a specific directory with explicit quantization level.
pub fn init_from_dir_with_quantization(
    model_dir: &Path,
    quantization: Quantization,
) -> Result<()> {
    let model = ParakeetModel::load(model_dir, &quantization)
        .map_err(|e| Error::Parakeet(format!("Failed to load Parakeet model: {e}")))?;
    log::info!(
        "Parakeet TDT model loaded from {} (quantization: {:?})",
        model_dir.display(),
        quantization,
    );
    let _ = PARAKEET_MODEL.set(Mutex::new(model));
    Ok(())
}

/// Run a short silent buffer to precondition the ONNX sessions.
pub fn warm_up() -> Result<()> {
    let silence = vec![0.0_f32; 25_600]; // 1.6 seconds of silence at 16kHz
    let model = PARAKEET_MODEL
        .get()
        .ok_or_else(|| Error::Parakeet("Parakeet model not initialized".into()))?;
    let mut model = model
        .lock()
        .map_err(|e| Error::Parakeet(format!("Parakeet model lock poisoned: {e}")))?;
    let _ = model
        .transcribe_with(&silence, &ParakeetParams::default())
        .map_err(|e| Error::Parakeet(format!("Parakeet warm-up failed: {e}")))?;
    log::info!("Parakeet warm-up completed");
    Ok(())
}

/// Transcribe a full 16kHz mono f32 audio buffer in a single pass.
pub fn transcribe(audio: &[f32]) -> Result<String> {
    let model = PARAKEET_MODEL
        .get()
        .ok_or_else(|| Error::Parakeet("Parakeet model not initialized".into()))?;
    let mut model = model
        .lock()
        .map_err(|e| Error::Parakeet(format!("Parakeet model lock poisoned: {e}")))?;
    log::info!("Parakeet using energy-adaptive transcription");
    let result = transcribe_energy_adaptive(&mut model, audio)?;
    Ok(result.text)
}
