use thiserror::Error;

pub use steam_vent_cdn::error::SteamVentError;

#[derive(Error, Debug)]
pub enum Error {
    #[error("CDN / Steam API error")]
    Cdn(#[from] SteamVentError),
    #[error(".tmod file error")]
    Tmod(#[from] TmodError),
    #[error("Localization file error")]
    Localization(#[from] LocalizationError),
    #[error("I/O error")]
    Io(#[from] std::io::Error),
}

#[derive(Error, Debug)]
pub enum TmodError {
    #[error("Invalid .tmod magic")]
    InvalidMagic,
    #[error("Entry offset {offset} exceeds file bounds (size: {file_size})")]
    EntryOffsetOutOfBounds { offset: u64, file_size: u64 },
    #[error("Unexpected end of data: expected {expected} bytes, got {actual}")]
    UnexpectedEof { expected: usize, actual: usize },
    #[error("Suspicious entry count: {0}")]
    InvalidEntryCount(i32),
    #[error("Entry '{0}' not found in .tmod archive")]
    EntryNotFound(String),
}

#[derive(Error, Debug)]
pub enum LocalizationError {
    #[error("Could not detect culture from path: {0}")]
    CultureNotFound(String),
    #[error("HJSON parse error")]
    HjsonParse(#[from] hjson_edit::HjsonEditError),
}
