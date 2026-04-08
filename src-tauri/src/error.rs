use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Audio error: {0}")]
    Audio(String),
    #[error("ORT error: {0}")]
    Ort(String),
    #[error("Parakeet error: {0}")]
    Parakeet(String),
    #[error("Paste error: {0}")]
    Paste(String),
    #[error("Hotkey error: {0}")]
    Hotkey(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Tauri(#[from] tauri::Error),
}

impl Serialize for Error {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::ser::Serializer,
    {
        serializer.serialize_str(self.to_string().as_ref())
    }
}

pub type Result<T> = std::result::Result<T, Error>;
