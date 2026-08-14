use std::path::{Component, Path};

use hjson_edit::{Format, HjsonDocument, HjsonDocumentMerge, HjsonObjectPath};
use tabled::{builder::Builder, settings::Style};
use tml_workshop::{CdnCache, CulturePrefix, LocalizationFile, WorkshopClient};

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

pub async fn run(args: LocalizeArgs, cache: Option<CdnCache>) -> Result<(), CliError> {
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

    let mut entries: Vec<&tml_workshop::FileEntry> = header
        .entries
        .iter()
        .filter(|e| is_localization_entry(&e.name))
        .collect();
    entries.sort_by(|a, b| a.name.cmp(&b.name));

    let mut files: Vec<Localized> = Vec::new();
    for entry in entries {
        let prefix = CulturePrefix::parse_with(&entry.name, CLI_CULTURES).ok_or_else(|| {
            CliError::msg(format!(
                "failed to detect culture for localization '{}'",
                entry.name
            ))
        })?;
        if prefix.culture != args.culture {
            continue;
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
        return Err(CliError::msg(format!(
            "no localization file found for culture {}",
            args.culture
        )));
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

    let merged = merge_files(files)?;
    print_merged(&merged)
}

fn merge_files(files: &[Localized]) -> Result<HjsonDocument, CliError> {
    let merge_error =
        |e: hjson_edit::HjsonEditError| CliError::msg(format!("failed to merge localization: {e}"));
    let mut doc = HjsonDocument::parse("{}").map_err(merge_error)?;
    for file in files {
        let prefix = file.loc.culture_prefix().prefix.as_str();
        let path = HjsonObjectPath::parse(prefix).map_err(merge_error)?;
        doc = doc
            .merge_from(&path, file.loc.document())
            .map_err(merge_error)?;
    }
    Ok(doc)
}

fn print_merged(merged: &HjsonDocument) -> Result<(), CliError> {
    println!("{}", render_merged(merged)?);
    Ok(())
}

fn render_merged(merged: &HjsonDocument) -> Result<String, CliError> {
    let render_error = |e: hjson_edit::HjsonEditError| {
        CliError::msg(format!("failed to render merged hjson: {e}"))
    };
    let ast = merged.ast().map_err(render_error)?;
    let formatted = ast.format(&hjson_edit::FormatConfig::default());
    Ok(hjson_edit::RedNode::new_root(formatted).text().to_string())
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

#[cfg(test)]
mod tests {
    use super::*;
    use hjson_edit::{HjsonDocumentObjectEdit, HjsonValue};

    fn localized(path: &str, content: &str) -> Localized {
        let loc = LocalizationFile::parse_with(path, content, CLI_CULTURES).expect("parse");
        Localized {
            path: path.to_string(),
            raw: content.to_string(),
            loc,
        }
    }

    fn value(doc: &HjsonDocument, dotted: &str) -> HjsonValue {
        doc.get(HjsonObjectPath::parse(dotted).unwrap())
            .expect("value")
    }

    #[test]
    fn merge_distinct_prefixes_combine_subtrees() {
        let files = vec![
            localized(
                "Localization/ja-JP_Mods.Alpha.hjson",
                r#"{ Items: { Sword: { DisplayName: "剣" } } }"#,
            ),
            localized(
                "Localization/ja-JP_Mods.Beta.hjson",
                r#"{ Items: { Shield: { DisplayName: "盾" } } }"#,
            ),
        ];
        let merged = merge_files(&files).expect("merge");
        assert_eq!(
            value(&merged, "Mods.Alpha.Items.Sword.DisplayName"),
            HjsonValue::String("剣".to_string())
        );
        assert_eq!(
            value(&merged, "Mods.Beta.Items.Shield.DisplayName"),
            HjsonValue::String("盾".to_string())
        );
    }

    #[test]
    fn merge_prefixed_file_overrides_base() {
        let files = vec![
            localized(
                "Localization/ja-JP.hjson",
                r#"{ Header: { Author: "me" }, Mods: { ExternalLocalizer: { Configs: { DisplayName: "base" } } } }"#,
            ),
            localized(
                "Localization/ja-JP_Mods.ExternalLocalizer.hjson",
                r#"{ Configs: { DisplayName: "override" } }"#,
            ),
        ];
        let merged = merge_files(&files).expect("merge");
        assert_eq!(
            value(&merged, "Header.Author"),
            HjsonValue::String("me".to_string())
        );
        assert_eq!(
            value(&merged, "Mods.ExternalLocalizer.Configs.DisplayName"),
            HjsonValue::String("override".to_string())
        );
    }

    #[test]
    fn merge_duplicate_leaf_later_file_wins() {
        let files = vec![
            localized(
                "Localization/ja-JP_Mods.Example.hjson",
                r#"{ Items: { A: { DisplayName: "first" } } }"#,
            ),
            localized(
                "Localization/ja-JP_Mods.Example.hjson",
                r#"{ Items: { A: { DisplayName: "second" } } }"#,
            ),
        ];
        let merged = merge_files(&files).expect("merge");
        assert_eq!(
            value(&merged, "Mods.Example.Items.A.DisplayName"),
            HjsonValue::String("second".to_string())
        );
    }

    #[test]
    fn merge_object_entries_are_deep_merged() {
        let files = vec![
            localized(
                "Localization/ja-JP_Mods.Example.hjson",
                r#"{ Items: { A: { DisplayName: "a", Tooltip: "t" } } }"#,
            ),
            localized(
                "Localization/ja-JP_Mods.Example.hjson",
                r#"{ Items: { A: { DisplayName: "a2" } } }"#,
            ),
        ];
        let merged = merge_files(&files).expect("merge");
        assert_eq!(
            value(&merged, "Mods.Example.Items.A.DisplayName"),
            HjsonValue::String("a2".to_string())
        );
        assert_eq!(
            value(&merged, "Mods.Example.Items.A.Tooltip"),
            HjsonValue::String("t".to_string())
        );
    }

    #[test]
    fn merge_prefix_segment_colliding_with_scalar_is_error() {
        let files = vec![
            localized("Localization/ja-JP.hjson", r#"{ Mods: "not-an-object" }"#),
            localized("Localization/ja-JP_Mods.Example.hjson", r#"{ Items: {} }"#),
        ];
        let err = merge_files(&files).expect_err("should fail");
        assert!(
            err.to_string().contains("'Mods' is not an object"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn merge_root_scalar_is_error() {
        let files = vec![localized("Localization/ja-JP.hjson", "\"just a string\"")];
        let err = merge_files(&files).expect_err("should fail");
        assert!(
            err.to_string().contains("source root is not an object"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn render_merged_outputs_valid_single_document() {
        let files = vec![
            localized(
                "Localization/ja-JP.hjson",
                r#"{ Header: { Author: "me" } }"#,
            ),
            localized(
                "Localization/ja-JP_Mods.Example.hjson",
                r#"{ Items: { Sword: { DisplayName: "剣" } } }"#,
            ),
        ];
        let merged = merge_files(&files).expect("merge");
        let rendered = render_merged(&merged).expect("render");

        let doc = hjson_edit::HjsonDocument::parse(&rendered).expect("reparse");
        let roundtripped = doc
            .get(hjson_edit::HjsonObjectPath::parse("").unwrap())
            .expect("get root");
        let merged_root = merged
            .get(hjson_edit::HjsonObjectPath::parse("").unwrap())
            .expect("get root");
        assert_eq!(roundtripped, merged_root);

        assert!(
            rendered.contains("\"me\""),
            "values should be quoted:\n{rendered}"
        );
        assert!(
            rendered.contains("\"剣\""),
            "values should be quoted:\n{rendered}"
        );
        assert!(
            !rendered.contains("# === "),
            "merged output must not contain file separators:\n{rendered}"
        );
    }

    #[test]
    fn merge_numeric_keyed_members_preserved() {
        let files = vec![
            localized(
                "Localization/en-US.hjson",
                r#"{ Mods: { Example: { Header: { Author: "me" } } } }"#,
            ),
            localized(
                "Localization/en-US_Mods.Example.hjson",
                r#"{ Chat: { 0: "Oink!", 1: "Squeal...", 2: "Snort!" } }"#,
            ),
        ];
        let merged = merge_files(&files).expect("merge");
        assert_eq!(
            value(&merged, "Mods.Example.Header.Author"),
            HjsonValue::String("me".to_string())
        );
        assert_eq!(
            value(&merged, "Mods.Example.Chat.0"),
            HjsonValue::String("Oink!".to_string())
        );
        assert_eq!(
            value(&merged, "Mods.Example.Chat.1"),
            HjsonValue::String("Squeal...".to_string())
        );
        assert_eq!(
            value(&merged, "Mods.Example.Chat.2"),
            HjsonValue::String("Snort!".to_string())
        );
    }

    #[test]
    fn render_merged_with_numeric_keys_roundtrips() {
        let files = vec![
            localized(
                "Localization/en-US_Mods.Example.hjson",
                r#"{ Chat: { 0: "Oink!", 1: "Squeal..." } }"#,
            ),
            localized("Localization/en-US_Mods.Other.hjson", r#"{ Title: "Hi" }"#),
        ];
        let merged = merge_files(&files).expect("merge");
        let rendered = render_merged(&merged).expect("render");

        let doc = hjson_edit::HjsonDocument::parse(&rendered).expect("reparse");
        let roundtripped = doc
            .get(hjson_edit::HjsonObjectPath::parse("").unwrap())
            .expect("get root");
        let merged_root = merged
            .get(hjson_edit::HjsonObjectPath::parse("").unwrap())
            .expect("get root");
        assert_eq!(roundtripped, merged_root);
        assert!(
            rendered.contains("0: \"Oink!\""),
            "numeric keys and quoted values should be preserved:\n{rendered}"
        );
    }

    #[test]
    fn merge_preserves_source_quote_styles() {
        let files = vec![
            localized(
                "Localization/en-US.hjson",
                r#"{ Header: { Author: "me" } }"#,
            ),
            localized(
                "Localization/en-US_Mods.Example.hjson",
                "{ Items: {\n  Sword: plain\n  Shield: \"quoted\"\n} }",
            ),
        ];
        let merged = merge_files(&files).expect("merge");
        let rendered = render_merged(&merged).expect("render");
        assert!(
            rendered.contains("Sword: plain"),
            "unquoted value style lost:\n{rendered}"
        );
        assert!(
            rendered.contains("Shield: \"quoted\""),
            "quoted value style lost:\n{rendered}"
        );
    }
}
