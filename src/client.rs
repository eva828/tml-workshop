use std::sync::Arc;

use futures::StreamExt;
use steam_vent_cdn::{
    config::Config,
    connection::SteamClient,
    downloader::{CdnDownloader, CdnRandomAccessReader, Manifest, ManifestEntry, ProgressCallback},
    error::{SteamVentError, WorkshopError},
};
use tokio::io::AsyncWrite;

use crate::error::Error;
use crate::tmod::{TmodReader, Version};
use crate::{ItemId, ManifestId, VersionCache};

const APP_ID: u32 = 1281930;

struct NoopProgress;

impl ProgressCallback for NoopProgress {
    fn on_download_progress(&self, _bytes: u64) {}
    fn on_decode_progress(&self, _bytes: u64) {}
    fn on_write_progress(&self, _bytes: u64) {}
}

pub struct WorkshopClient {
    downloader: Arc<CdnDownloader>,
    cache: VersionCache,
}

impl WorkshopClient {
    pub async fn connect() -> Result<Self, Error> {
        let config = Config::builder().app_id(APP_ID).build();
        let client = SteamClient::new(config);
        let session = client.connect().await?;
        let downloader = CdnDownloader::connect(session).await?;
        Ok(Self {
            downloader: Arc::new(downloader),
            cache: VersionCache::new(),
        })
    }

    pub fn with_cache(self, cache: VersionCache) -> Self {
        Self { cache, ..self }
    }

    pub fn cache(&self) -> &VersionCache {
        &self.cache
    }

    pub async fn fetch_item_details(
        &self,
        item_id: ItemId,
    ) -> Result<steam_vent_cdn::downloader::ItemDetails, Error> {
        Ok(self.downloader.fetch_item_details(item_id.raw()).await?)
    }

    pub async fn fetch_latest_manifest_id(&self, item_id: ItemId) -> Result<ManifestId, Error> {
        let raw = self
            .downloader
            .fetch_latest_manifest_id(item_id.raw())
            .await?;
        Ok(ManifestId::new(raw))
    }

    pub async fn fetch_manifest_by_id(&self, manifest_id: ManifestId) -> Result<Manifest, Error> {
        Ok(self
            .downloader
            .fetch_manifest_by_id(manifest_id.raw(), APP_ID)
            .await?)
    }

    pub fn reader_for(
        &self,
        entry: &ManifestEntry,
    ) -> Result<TmodReader<CdnRandomAccessReader>, Error> {
        let progress = Arc::new(NoopProgress);
        let reader = self.downloader.create_reader(entry, progress)?;
        Ok(TmodReader::new(reader))
    }

    /// Streams the full `.tmod` file (header + all entries) from the CDN
    /// into `dest`, returning the total number of bytes written.
    pub async fn save_tmod(
        &self,
        entry: &ManifestEntry,
        dest: impl AsyncWrite + Unpin,
    ) -> Result<u64, Error> {
        let progress = Arc::new(NoopProgress);
        let mut reader = self.downloader.create_reader(entry, progress)?;
        let mut dest = dest;
        Ok(tokio::io::copy(&mut reader, &mut dest).await?)
    }

    pub async fn pick_latest_tmod(
        &self,
        manifest: &Manifest,
    ) -> Result<Option<ManifestEntry>, Error> {
        let tmod_files: Vec<&ManifestEntry> = manifest
            .files
            .iter()
            .filter(|f| f.filename.ends_with(".tmod"))
            .collect();

        match tmod_files.len() {
            0 => Ok(None),
            1 => Ok(Some(tmod_files[0].clone())),
            _ => {
                let mut best: Option<(Version, ManifestEntry)> = None;
                for entry in &tmod_files {
                    let mut tmod = self.reader_for(entry)?;
                    let header = tmod.parse_header().await?;
                    let ver = Version::parse(&header.tml_version)?;
                    match &best {
                        None => best = Some((ver, (*entry).clone())),
                        Some((current_ver, _)) if ver > *current_ver => {
                            best = Some((ver, (*entry).clone()));
                        }
                        _ => {}
                    }
                }
                Ok(best.map(|(_, entry)| entry))
            }
        }
    }

    pub async fn find_item_by_slug(&self, slug: &str) -> Result<ItemId, Error> {
        let raw = self.downloader.fetch_item_id_by_slug(APP_ID, slug).await?;
        Ok(ItemId::new(raw))
    }

    async fn resolve_mod_version(&self, manifest_id: ManifestId) -> Result<Option<Version>, Error> {
        if let Some(cached) = self.cache.get(manifest_id) {
            return Ok(Some(Version::parse(&cached)?));
        }

        let manifest = self
            .downloader
            .fetch_manifest_by_id(manifest_id.raw(), APP_ID)
            .await?;

        let tmod_entry = match self.pick_latest_tmod(&manifest).await? {
            Some(e) => e,
            None => return Ok(None),
        };

        let progress = Arc::new(NoopProgress);
        let reader = self.downloader.create_reader(&tmod_entry, progress)?;
        let mut tmod = TmodReader::new(reader);
        let header = tmod.parse_header().await?;

        let version = Version::parse(&header.mod_version)?;
        self.cache.insert(manifest_id, header.mod_version);
        Ok(Some(version))
    }

    pub async fn find_version(&self, item_id: ItemId, version: Version) -> Result<ManifestId, Error> {
        let mut stream = self.downloader.fetch_versions(item_id.raw(), None);

        while let Some(version_info) = stream.next().await {
            let version_info = version_info?;
            let manifest_id = ManifestId::new(version_info.manifest_id);

            if let Some(ver) = self.resolve_mod_version(manifest_id).await?
                && ver == version
            {
                return Ok(manifest_id);
            }
        }

        Err(SteamVentError::Workshop(WorkshopError::VersionNotFound(version.to_string())).into())
    }

    pub async fn list_versions(
        &self,
        item_id: ItemId,
        limit: Option<u32>,
    ) -> Result<Vec<Version>, Error> {
        let mut stream = self.downloader.fetch_versions(item_id.raw(), limit);
        let mut versions = Vec::new();

        while let Some(version_info) = stream.next().await {
            let version_info = version_info?;
            let manifest_id = ManifestId::new(version_info.manifest_id);

            if let Some(ver) = self.resolve_mod_version(manifest_id).await? {
                versions.push(ver);
            }
        }

        Ok(versions)
    }
}
