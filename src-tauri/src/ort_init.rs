use crate::error::{Error, Result};
use crate::manifest_dir;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

static ORT_INIT_RESULT: OnceLock<std::result::Result<PathBuf, String>> = OnceLock::new();

pub fn find_ort_dylib(resource_dir: Option<&Path>) -> Result<PathBuf> {
    if let Some(path) = std::env::var_os("ORT_DYLIB_PATH") {
        let path = PathBuf::from(path);
        if path.exists() {
            return Ok(path);
        }
        log::warn!(
            "ORT_DYLIB_PATH points to a missing file: {}",
            path.display()
        );
    }

    let candidates = [
        resource_dir.map(|d| d.join("libonnxruntime.dylib")),
        Some(manifest_dir().join("resources/libonnxruntime.dylib")),
    ];

    for candidate in candidates.into_iter().flatten() {
        if candidate.exists() {
            return Ok(candidate);
        }
    }

    Err(Error::Ort(
        "ONNX Runtime dylib not found. Run scripts/download-models.sh to install it.".into(),
    ))
}

pub fn init(resource_dir: Option<&Path>) -> Result<PathBuf> {
    match ORT_INIT_RESULT.get_or_init(|| {
        let dylib = find_ort_dylib(resource_dir).map_err(|e| e.to_string())?;
        unsafe {
            std::env::set_var("ORT_DYLIB_PATH", &dylib);
        }
        if std::env::var_os("SKIP_REAL_ORT_INIT").is_some() {
            log::debug!(
                "Skipping ONNX Runtime initialization (SKIP_REAL_ORT_INIT set): {}",
                dylib.display()
            );
            return Ok(dylib);
        }
        let intra_threads = std::env::var("ORT_INTRA_THREADS")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(10);
        let thread_pool = ort::environment::GlobalThreadPoolOptions::default()
            .with_intra_threads(intra_threads)
            .map_err(|e| format!("Failed to configure thread pool: {e}"))?;
        ort::init_from(&dylib)
            .map_err(|e| format!("Failed to initialize ONNX Runtime: {e}"))?
            .with_global_thread_pool(thread_pool)
            .commit();
        Ok(dylib)
    }) {
        Ok(path) => Ok(path.clone()),
        Err(err) => Err(Error::Ort(err.clone())),
    }
}
