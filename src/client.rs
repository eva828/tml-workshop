use std::sync::Arc;

use futures::StreamExt;
use steam_vent_cdn::{
    config::Config,
    connection::SteamClient,
    downloader::{CdnDownloader, CdnRandomAccessReader, Manifest, ManifestEntry, ProgressCallback},
    error::{SteamVentError, WorkshopError},
};

use crate::error::Error;
use crate::tmod::{parse_tml_version, TmodReader, TmlVersion};
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

    pub async fn fetch_manifest_info(
        &self,
        item_id: ItemId,
        manifest_id: ManifestId,
    ) -> Result<Manifest, Error> {
        let details = self.downloader.fetch_item_details(item_id.raw()).await?;
        let app_id = details.consumer_appid.unwrap_or(APP_ID);
        Ok(self
            .downloader
            .fetch_manifest_by_id(manifest_id.raw(), app_id)
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
                let mut best: Option<(TmlVersion, ManifestEntry)> = None;
                for entry in &tmod_files {
                    let mut tmod = self.reader_for(entry)?;
                    let header = tmod.parse_header().await?;
                    let ver = parse_tml_version(&header.tml_version);
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

    pub async fn find_version(&self, item_id: ItemId, version: &str) -> Result<ManifestId, Error> {
        let mut stream = self.downloader.fetch_versions(item_id.raw(), None);

        while let Some(version_info) = stream.next().await {
            let version_info = version_info?;
            let manifest_id = ManifestId::new(version_info.manifest_id);

            if let Some(cached_ver) = self.cache.get(manifest_id) {
                if cached_ver == version {
                    return Ok(manifest_id);
                }
                continue;
            }

            let manifest = self
                .downloader
                .fetch_manifest_by_id(manifest_id.raw(), APP_ID)
                .await?;

            let tmod_entry = match self.pick_latest_tmod(&manifest).await? {
                Some(e) => e,
                None => continue,
            };

            let progress = Arc::new(NoopProgress);
            let reader = self.downloader.create_reader(&tmod_entry, progress)?;
            let mut tmod = TmodReader::new(reader);
            let header = tmod.parse_header().await?;

            let peeked = header.mod_version;
            self.cache.insert(manifest_id, peeked.clone());
            if peeked == version {
                return Ok(manifest_id);
            }
        }

        Err(SteamVentError::Workshop(WorkshopError::VersionNotFound(version.to_string())).into())
    }
}
