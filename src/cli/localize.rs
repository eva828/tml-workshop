use std::path::{Component, Path};

use tabled::{builder::Builder, settings::Style};
use tml_workshop::{CulturePrefix, LocalizationFile, WorkshopClient};

use super::CliError;
use super::args::{LocalizeArgs, OutputFormat};

const CLI_CULTURES: &[&str] = &[
    "en-US", "de-DE", "it-IT", "fr-FR", "zh-Hans", "es-ES", "ru-RU", "pt-BR", "pl-PL", "zh-Hant",
    "tr-TR", "uk-UA", "ko-KR", "ja-JP", "vi-VN", "th-TH", "pt-PT", "cs-CZ", "hu-HU", "sv-SE",
    "nl-NL", "nb-NO", "da-DK", "fi-FI", "ro-RO", "el-GR", "hr-HR", "sk-SK", "sr-SR", "bg-BG",
    "ar-SA", "he-IL", "id-ID", "ms-MY",
];

struct Localized {
    path: String,
    raw: String,
    loc: LocalizationFile,
}

pub async fn run(args: LocalizeArgs) -> Result<(), CliError> {
    let client = WorkshopClient::connect().await?;
    let item_id = super::resolve_item(&client, &args.item).await?;
    let manifest_id = client.fetch_latest_manifest_id(item_id).await?;
    let manifest = client.fetch_manifest_by_id(manifest_id).await?;
    let entry = client
        .pick_latest_tmod(&manifest)
        .await?
        .ok_or_else(|| CliError::msg(format!("no .tmod entry found in manifest {manifest_id}")))?;

    let mut tmod = client.reader_for(&entry)?;
    let header = tmod.parse_header().await?;

    let mut entries: Vec<&tml_workshop::FileEntry> = header
        .entries
        .iter()
        .filter(|e| is_localization_entry(&e.name))
        .collect();
    entries.sort_by(|a, b| a.name.cmp(&b.name));

    let mut files: Vec<Localized> = Vec::new();
    for entry in entries {
        if let Some(culture) = &args.culture {
            let prefix = CulturePrefix::parse_with(&entry.name, CLI_CULTURES).ok_or_else(|| {
                CliError::msg(format!(
                    "failed to detect culture for localization '{}'",
                    entry.name
                ))
            })?;
            if prefix.culture != *culture {
                continue;
            }
        }

        let raw = tmod.extract_file_as_text(entry).await?;
        let loc = LocalizationFile::parse_with(&entry.name, &raw, CLI_CULTURES).map_err(|e| {
            CliError::msg(format!(
                "failed to parse localization '{}': {e}",
                entry.name
            ))
        })?;
        files.push(Localized {
            path: entry.name.clone(),
            raw,
            loc,
        });
    }

    if files.is_empty() {
        match &args.culture {
            Some(culture) => {
                return Err(CliError::msg(format!(
                    "no localization file found for culture {culture}"
                )));
            }
            None => return Err(CliError::msg("no localization files found in the mod")),
        }
    }

    match args.format {
        OutputFormat::Hjson => output_hjson(&files),
        OutputFormat::Json => output_json(&files),
        OutputFormat::Csv => output_csv(&files),
        OutputFormat::Table => output_table(&files),
    }
}

fn is_localization_entry(name: &str) -> bool {
    let path = Path::new(name);
    let has_localization_dir = path
        .components()
        .any(|c| matches!(c, Component::Normal(s) if s == "Localization"));
    let extension = path.extension().and_then(|e| e.to_str());
    has_localization_dir && matches!(extension, Some("hjson") | Some("json"))
}

fn output_hjson(files: &[Localized]) -> Result<(), CliError> {
    if files.len() == 1 {
        print_raw(&files[0].raw);
        return Ok(());
    }

    for file in files {
        println!("# === {} ===", file.path);
        print_raw(&file.raw);
    }
    Ok(())
}

fn output_json(files: &[Localized]) -> Result<(), CliError> {
    let mut root = serde_json::Map::new();
    for file in files {
        let mut entries = serde_json::Map::new();
        for (key, value) in flatten_sorted(&file.loc)? {
            entries.insert(key, serde_json::Value::String(value));
        }

        let mut file_obj = serde_json::Map::new();
        file_obj.insert(
            "culture".to_string(),
            serde_json::Value::String(file.loc.culture_prefix().culture.clone()),
        );
        file_obj.insert(
            "prefix".to_string(),
            serde_json::Value::String(file.loc.culture_prefix().prefix.clone()),
        );
        file_obj.insert("entries".to_string(), serde_json::Value::Object(entries));
        root.insert(file.path.clone(), serde_json::Value::Object(file_obj));
    }
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::Value::Object(root))?
    );
    Ok(())
}

fn output_csv(files: &[Localized]) -> Result<(), CliError> {
    let mut writer = csv::Writer::from_writer(std::io::stdout());
    writer.write_record(["path", "culture", "prefix", "key", "value"])?;

    for file in files {
        for (key, value) in flatten_sorted(&file.loc)? {
            writer.write_record([
                file.path.as_str(),
                file.loc.culture_prefix().culture.as_str(),
                file.loc.culture_prefix().prefix.as_str(),
                key.as_str(),
                value.as_str(),
            ])?;
        }
    }
    writer.flush()?;
    Ok(())
}

fn output_table(files: &[Localized]) -> Result<(), CliError> {
    let mut builder = Builder::new();
    builder.push_record(["path", "culture", "key", "value"]);

    for file in files {
        for (key, value) in flatten_sorted(&file.loc)? {
            builder.push_record([
                file.path.as_str(),
                file.loc.culture_prefix().culture.as_str(),
                key.as_str(),
                value.as_str(),
            ]);
        }
    }

    let mut table = builder.build();
    table.with(Style::modern());
    println!("{table}");
    Ok(())
}

fn flatten_sorted(loc: &LocalizationFile) -> Result<Vec<(String, String)>, CliError> {
    let mut flat: Vec<(String, String)> = loc
        .flatten()
        .map_err(tml_workshop::Error::from)?
        .into_iter()
        .collect();
    flat.sort_unstable();
    Ok(flat)
}

fn print_raw(text: &str) {
    print!("{text}");
    if !text.ends_with('\n') {
        println!();
    }
}
