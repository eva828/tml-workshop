use std::path::{Component, Path, PathBuf};
use std::pin::Pin;
use std::task::{Context, Poll};

use indicatif::{ProgressBar, ProgressStyle};
use tml_workshop::{
    CdnCache, CdnRandomAccessReader, ItemId, ManifestId, TmodHeader, TmodReader, WorkshopClient,
};
use tokio::io::AsyncWrite;

use super::CliError;
use super::args::DownloadArgs;

pub async fn run(args: DownloadArgs, cache: Option<CdnCache>) -> Result<(), CliError> {
    let client = WorkshopClient::connect(cache).await?;
    let item_id = super::resolve_item(&client, &args.item).await?;

    let manifest_id =
        super::select_manifest_id(&client, item_id, args.version.as_deref(), args.manifest_id)
            .await?;
    let manifest = client.fetch_manifest_by_id(manifest_id).await?;
    let entry = client
        .pick_latest_tmod(&manifest)
        .await?
        .ok_or_else(|| CliError::msg(format!("no .tmod entry found in manifest {manifest_id}")))?;

    let mut tmod = client.reader_for(&entry)?;
    let header = tmod.parse_header().await?;

    let context = OutputContext {
        item_id,
        manifest_id,
        mod_name: header.mod_name.clone(),
        mod_version: header.mod_version.clone(),
    };
    let output = PathBuf::from(render_template(&args.output, &context)?);

    if let Some(parent) = output.parent()
        && !parent.as_os_str().is_empty()
    {
        tokio::fs::create_dir_all(parent).await?;
    }

    let total = entry.size.unwrap_or_else(|| tmod_total_size(&header));
    let bar = ProgressBar::new(total);
    bar.set_style(
        ProgressStyle::with_template(
            "{spinner:.cyan} {msg} [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({percent}%) {eta}",
        )
        .unwrap()
        .progress_chars("=>-"),
    );
    bar.set_message("Downloading .tmod");
    bar.enable_steady_tick(std::time::Duration::from_millis(100));

    let file = tokio::fs::File::create(&output).await?;
    let writer = CountingWriter::new(file, Some(bar.clone()));
    let bytes = client.save_tmod(&entry, writer).await?;
    bar.finish_and_clear();
    println!("Saved {} ({} bytes)", output.display(), bytes);

    if args.extract {
        extract(&mut tmod, &header, &output).await?;
    }

    Ok(())
}

struct OutputContext {
    item_id: ItemId,
    manifest_id: ManifestId,
    mod_name: String,
    mod_version: String,
}

fn render_template(template: &str, context: &OutputContext) -> Result<String, CliError> {
    let mut result = String::new();
    let mut rest = template;

    while let Some(start) = rest.find('{') {
        result.push_str(&rest[..start]);

        let after = &rest[start + 1..];
        let Some(end) = after.find('}') else {
            return Err(CliError::msg(format!(
                "unclosed '{{' in output template: {template}"
            )));
        };

        let name = &after[..end];
        let value = match name {
            "modname" => context.mod_name.clone(),
            "modversion" => context.mod_version.clone(),
            "itemid" => context.item_id.raw().to_string(),
            "manifestid" => context.manifest_id.raw().to_string(),
            _ => {
                return Err(CliError::msg(format!(
                    "unknown placeholder {{{name}}} in output template: {template}"
                )));
            }
        };
        validate_path_component(&value)?;
        result.push_str(&value);
        rest = &after[end + 1..];
    }

    result.push_str(rest);
    Ok(result)
}

fn validate_path_component(value: &str) -> Result<(), CliError> {
    const INVALID_CHARS: &[char] = &['\\', '/', ':', '*', '?', '"', '<', '>', '|'];

    if value.contains(INVALID_CHARS) {
        return Err(CliError::msg(format!(
            "value {value:?} contains characters invalid in a path component"
        )));
    }
    if value.chars().any(|c| c.is_control()) {
        return Err(CliError::msg(format!(
            "value {value:?} contains control characters"
        )));
    }
    Ok(())
}

fn tmod_total_size(header: &TmodHeader) -> u64 {
    header
        .entries
        .last()
        .map(|e| e.offset + e.compressed_size as u64)
        .unwrap_or(0)
}

async fn extract(
    tmod: &mut TmodReader<CdnRandomAccessReader>,
    header: &TmodHeader,
    output: &Path,
) -> Result<(), CliError> {
    validate_path_component(&header.mod_name)?;

    let extract_dir = output
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(&header.mod_name);
    tokio::fs::create_dir_all(&extract_dir).await?;

    for entry in &header.entries {
        validate_extract_entry(&entry.name)?;
        let data = tmod.extract_file(entry).await?;
        let dest = extract_dir.join(&entry.name);
        if let Some(parent) = dest.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::fs::write(&dest, data).await?;
    }

    println!(
        "Extracted {} files to {}",
        header.entries.len(),
        extract_dir.display()
    );
    Ok(())
}

fn validate_extract_entry(name: &str) -> Result<(), CliError> {
    if name.is_empty() {
        return Err(CliError::msg("empty entry name in .tmod"));
    }

    let path = Path::new(name);
    for component in path.components() {
        match component {
            Component::ParentDir => {
                return Err(CliError::msg(format!(
                    "unsafe entry path in .tmod (contains '..'): {name}"
                )));
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err(CliError::msg(format!(
                    "unsafe absolute entry path in .tmod: {name}"
                )));
            }
            Component::CurDir | Component::Normal(_) => {}
        }
    }
    Ok(())
}

struct CountingWriter<W> {
    inner: W,
    bar: Option<ProgressBar>,
}

impl<W> CountingWriter<W> {
    fn new(inner: W, bar: Option<ProgressBar>) -> Self {
        Self { inner, bar }
    }
}

impl<W: AsyncWrite + Unpin> AsyncWrite for CountingWriter<W> {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        let written = Pin::new(&mut self.inner).poll_write(cx, buf);
        if let Poll::Ready(Ok(n)) = written
            && let Some(bar) = &self.bar
        {
            bar.inc(n as u64);
        }
        written
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}

impl<W: Unpin> Unpin for CountingWriter<W> {}
