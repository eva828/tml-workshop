use crate::error::{Error, TmodError};
use steam_vent_cdn::downloader::{Manifest, ManifestEntry};
use tokio::io::{AsyncReadExt, AsyncSeekExt, SeekFrom};

const TMOD_MAGIC: &[u8; 4] = b"TMOD";

#[derive(Debug, Clone)]
pub struct FileEntry {
    pub name: String,
    pub uncompressed_size: u32,
    pub compressed_size: u32,
    pub offset: u64,
}

#[derive(Debug, Clone)]
pub struct TmodHeader {
    pub tml_version: String,
    pub mod_name: String,
    pub mod_version: String,
    pub hash: [u8; 20],
    pub entries: Vec<FileEntry>,
}

pub struct TmodReader<R> {
    inner: R,
}

impl<R> TmodReader<R> {
    pub fn new(reader: R) -> Self {
        Self { inner: reader }
    }
}

impl<R: tokio::io::AsyncRead + tokio::io::AsyncSeek + Unpin + Send> TmodReader<R> {
    pub async fn parse_header(&mut self) -> Result<TmodHeader, Error> {
        self.inner.seek(SeekFrom::Start(0)).await?;

        let mut magic = [0u8; 4];
        self.inner.read_exact(&mut magic).await?;
        if magic != *TMOD_MAGIC {
            return Err(TmodError::InvalidMagic.into());
        }

        let _tml_version = read_string(&mut self.inner).await?;

        let mut hash = [0u8; 20];
        self.inner.read_exact(&mut hash).await?;

        let mut sig = [0u8; 256];
        self.inner.read_exact(&mut sig).await?;

        let _file_data_len = read_u32le(&mut self.inner).await?;

        let mod_name = read_string(&mut self.inner).await?;
        let mod_version = read_string(&mut self.inner).await?;

        let entry_count = read_i32le(&mut self.inner).await?;
        if !(0..=10_000).contains(&entry_count) {
            return Err(TmodError::InvalidEntryCount(entry_count).into());
        }

        let mut entries = Vec::with_capacity(entry_count as usize);
        for _ in 0..entry_count {
            let name = read_string(&mut self.inner).await?;
            let uncompressed_size = read_u32le(&mut self.inner).await?;
            let compressed_size = read_u32le(&mut self.inner).await?;
            entries.push(FileEntry {
                name,
                uncompressed_size,
                compressed_size,
                offset: 0,
            });
        }

        let data_start = self.inner.stream_position().await?;
        let mut offset = data_start;
        for entry in &mut entries {
            entry.offset = offset;
            offset += entry.compressed_size as u64;
        }

        Ok(TmodHeader {
            tml_version: _tml_version,
            mod_name,
            mod_version,
            hash,
            entries,
        })
    }

    pub async fn extract_file(&mut self, entry: &FileEntry) -> Result<Vec<u8>, Error> {
        self.inner.seek(SeekFrom::Start(entry.offset)).await?;

        let mut compressed = vec![0u8; entry.compressed_size as usize];
        self.inner.read_exact(&mut compressed).await?;

        if entry.compressed_size == entry.uncompressed_size {
            return Ok(compressed);
        }

        let mut decompressed = Vec::with_capacity(entry.uncompressed_size as usize);
        let mut decoder = flate2::read::DeflateDecoder::new(&compressed[..]);
        std::io::Read::read_to_end(&mut decoder, &mut decompressed)?;
        Ok(decompressed)
    }

    pub async fn extract_file_as_text(&mut self, entry: &FileEntry) -> Result<String, Error> {
        let bytes = self.extract_file(entry).await?;
        String::from_utf8(bytes)
            .map_err(|e| Error::Io(std::io::Error::new(std::io::ErrorKind::InvalidData, e)))
    }
}

async fn read_7bit_int<R: tokio::io::AsyncRead + Unpin>(reader: &mut R) -> Result<i32, Error> {
    let mut result: u32 = 0;
    let mut shift = 0u32;

    loop {
        if shift >= 35 {
            return Err(Error::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "7-bit encoded integer overflow",
            )));
        }

        let mut buf = [0u8; 1];
        reader.read_exact(&mut buf).await?;
        let byte = buf[0];

        result |= ((byte & 0x7F) as u32) << shift;
        shift += 7;

        if byte & 0x80 == 0 {
            break;
        }
    }

    Ok(result as i32)
}

async fn read_string<R: tokio::io::AsyncRead + Unpin>(reader: &mut R) -> Result<String, Error> {
    let len = read_7bit_int(reader).await?;
    if len <= 0 {
        return Ok(String::new());
    }

    let mut buf = vec![0u8; len as usize];
    reader.read_exact(&mut buf).await?;
    String::from_utf8(buf)
        .map_err(|e| Error::Io(std::io::Error::new(std::io::ErrorKind::InvalidData, e)))
}

async fn read_i32le<R: tokio::io::AsyncRead + Unpin>(reader: &mut R) -> Result<i32, Error> {
    let mut buf = [0u8; 4];
    reader.read_exact(&mut buf).await?;
    Ok(i32::from_le_bytes(buf))
}

async fn read_u32le<R: tokio::io::AsyncRead + Unpin>(reader: &mut R) -> Result<u32, Error> {
    let mut buf = [0u8; 4];
    reader.read_exact(&mut buf).await?;
    Ok(u32::from_le_bytes(buf))
}

pub trait ManifestExt {
    fn pick_latest_tmod(&self) -> Option<&ManifestEntry>;
}

impl ManifestExt for Manifest {
    fn pick_latest_tmod(&self) -> Option<&ManifestEntry> {
        let tmod_files: Vec<&ManifestEntry> = self
            .files
            .iter()
            .filter(|f| f.filename.ends_with(".tmod"))
            .collect();

        match tmod_files.len() {
            0 => None,
            1 => Some(tmod_files[0]),
            _ => tmod_files.into_iter().max_by(|a, b| {
                let va = parse_tml_version_from_path(&a.filename);
                let vb = parse_tml_version_from_path(&b.filename);
                va.cmp(&vb)
            }),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct TmlVersion {
    year: u32,
    month: u32,
}

fn parse_tml_version_from_path(path: &str) -> TmlVersion {
    let normalized = path.replace('\\', "/");
    if let Some(parent) = std::path::Path::new(&normalized)
        .parent()
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        && let Some((year_str, month_str)) = parent.split_once('.')
        && let (Ok(year), Ok(month)) = (year_str.parse::<u32>(), month_str.parse::<u32>())
    {
        return TmlVersion { year, month };
    }
    TmlVersion { year: 0, month: 0 }
}
