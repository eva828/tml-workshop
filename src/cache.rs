use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;

use crate::{Error, ManifestId};

pub struct VersionCache {
    map: Mutex<HashMap<ManifestId, String>>,
}

impl VersionCache {
    pub fn new() -> Self {
        Self {
            map: Mutex::new(HashMap::new()),
        }
    }

    pub fn get(&self, manifest_id: ManifestId) -> Option<String> {
        self.map.lock().unwrap().get(&manifest_id).cloned()
    }

    pub fn insert(&self, manifest_id: ManifestId, version: String) {
        self.map.lock().unwrap().insert(manifest_id, version);
    }

    pub fn save(&self, path: impl AsRef<Path>) -> Result<(), Error> {
        let map = self.map.lock().unwrap();
        let string_map: HashMap<String, String> = map
            .iter()
            .map(|(k, v)| (k.raw().to_string(), v.clone()))
            .collect();
        let json = serde_json::to_string_pretty(&string_map)
            .map_err(|e| Error::Io(std::io::Error::new(std::io::ErrorKind::InvalidData, e)))?;
        std::fs::write(path, json)?;
        Ok(())
    }

    pub fn load(path: impl AsRef<Path>) -> Result<Self, Error> {
        let data = std::fs::read_to_string(path)?;
        let string_map: HashMap<String, String> = serde_json::from_str(&data)
            .map_err(|e| Error::Io(std::io::Error::new(std::io::ErrorKind::InvalidData, e)))?;
        let map: HashMap<ManifestId, String> = string_map
            .into_iter()
            .map(|(k, v)| {
                let raw: u64 = k.parse().map_err(|e| {
                    Error::Io(std::io::Error::new(std::io::ErrorKind::InvalidData, e))
                })?;
                Ok((ManifestId::new(raw), v))
            })
            .collect::<Result<_, Error>>()?;
        Ok(Self {
            map: Mutex::new(map),
        })
    }
}

impl Default for VersionCache {
    fn default() -> Self {
        Self::new()
    }
}
