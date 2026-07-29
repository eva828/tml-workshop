mod cache;
mod client;
mod error;
mod key;
mod localization;
mod tmod;

use std::fmt::{Display, Formatter, Result as FmtResult};

use serde::{Deserialize, Serialize};

pub use error::{Error, LocalizationError, TmodError};
pub use steam_vent_cdn::downloader::CdnRandomAccessReader;
pub use steam_vent_cdn::downloader::{
    ChunkInfo, ItemDetails, Manifest, ManifestEntry, VersionInfo,
};
pub use steam_vent_cdn::error::SteamVentError;
pub use cache::VersionCache;
pub use client::WorkshopClient;
pub use key::KeyParts;
pub use localization::{CulturePrefix, LocalizationFile, KNOWN_CULTURES};
pub use tmod::{FileEntry, TmodHeader, TmodReader};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ItemId(u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ManifestId(u64);

impl ItemId {
    pub fn new(raw: u64) -> Self {
        Self(raw)
    }

    pub fn raw(self) -> u64 {
        self.0
    }
}

impl Display for ItemId {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "{}", self.0)
    }
}

impl ManifestId {
    pub fn new(raw: u64) -> Self {
        Self(raw)
    }

    pub fn raw(self) -> u64 {
        self.0
    }
}

impl Display for ManifestId {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "{}", self.0)
    }
}
