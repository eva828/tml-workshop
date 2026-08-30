use thiserror::Error;

pub use steam_vent_cdn::error::SteamVentError;

#[derive(Error, Debug)]
pub enum Error {
    #[error("CDN / Steam API error: {0}")]
    Cdn(#[from] SteamVentError),
    #[error(".tmod file error")]
    Tmod(#[from] TmodError),
    #[error("Localization file error")]
    Localization(#[from] LocalizationError),
    #[error("I/O error")]
    Io(#[from] std::io::Error),
    #[error("Version parse error")]
    Version(#[from] VersionError),
}

impl Error {
    /// 対象のWorkshopアイテムが公開されていない（非公開・削除等）ために失敗したかを返す。
    ///
    /// この場合、翻訳ファイル（hjson）の更新は可能だがバージョン等の情報は取得できない。
    pub fn is_workshop_item_unavailable(&self) -> bool {
        match self {
            Error::Cdn(SteamVentError::Workshop(e)) => matches!(
                e,
                steam_vent_cdn::error::WorkshopError::ItemNotFoundBySlug(_)
                    | steam_vent_cdn::error::WorkshopError::GetDetailsFailed { .. }
                    | steam_vent_cdn::error::WorkshopError::ItemDetailsMissing(_)
                    | steam_vent_cdn::error::WorkshopError::NotAWorkshopItem
            ),
            _ => false,
        }
    }
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

#[derive(Error, Debug)]
pub enum VersionError {
    #[error("Must be 2 to 4 numbers separated by periods (got: {0:?})")]
    Invalid(String),
}
