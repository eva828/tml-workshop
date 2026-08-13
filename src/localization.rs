use std::collections::HashMap;

use hjson_edit::{HjsonDocument, RedNode, SyntaxKind};

use crate::error::LocalizationError;

pub const KNOWN_CULTURES: &[&str] = &[
    "en-US", "de-DE", "it-IT", "fr-FR", "zh-Hans", "es-ES", "ru-RU", "pt-BR", "pl-PL",
];

#[derive(Debug, Clone)]
pub struct CulturePrefix {
    pub culture: String,
    pub prefix: String,
}

impl CulturePrefix {
    pub fn parse(path: &str) -> Option<Self> {
        Self::parse_with(path, KNOWN_CULTURES)
    }

    pub fn parse_with(path: &str, cultures: &[&str]) -> Option<Self> {
        use std::path::Path;

        let path = Path::new(path).with_extension("");
        let path = path.to_string_lossy();
        let path = path.replace('\\', "/");

        let mut culture: Option<String> = None;

        for path_part in path.split('/') {
            let segments: Vec<&str> = path_part.split('_').collect();
            for (i, segment) in segments.iter().enumerate() {
                if cultures.contains(segment) {
                    culture = Some(segment.to_string());
                    continue;
                }
                if let Some(culture) = &culture {
                    let prefix = segments[i..].join("_");
                    return Some(CulturePrefix {
                        culture: culture.clone(),
                        prefix,
                    });
                }
            }
        }

        culture.map(|c| CulturePrefix {
            culture: c,
            prefix: String::new(),
        })
    }
}

pub struct LocalizationFile {
    culture_prefix: CulturePrefix,
    doc: HjsonDocument,
}

impl LocalizationFile {
    pub fn parse(path: &str, content: &str) -> Result<Self, LocalizationError> {
        Self::parse_with(path, content, KNOWN_CULTURES)
    }

    pub fn parse_with(
        path: &str,
        content: &str,
        cultures: &[&str],
    ) -> Result<Self, LocalizationError> {
        let culture_prefix = CulturePrefix::parse_with(path, cultures)
            .ok_or_else(|| LocalizationError::CultureNotFound(path.to_string()))?;
        let doc = HjsonDocument::parse(content)?;
        Ok(Self {
            culture_prefix,
            doc,
        })
    }

    pub fn culture_prefix(&self) -> &CulturePrefix {
        &self.culture_prefix
    }

    pub fn flatten(&self) -> Result<HashMap<String, String>, LocalizationError> {
        let root = self.doc.syntax_root();
        let mut result = HashMap::new();
        let mut path = Vec::new();
        collect_flat(root, &self.culture_prefix.prefix, &mut path, &mut result);
        Ok(result)
    }

    /// ファイルのルート値を HjsonValue として返す（配列・数値なども構造を保持）。
    pub fn document(&self) -> &HjsonDocument {
        &self.doc
    }
}

fn collect_flat(
    node: &RedNode,
    prefix: &str,
    path: &mut Vec<String>,
    result: &mut HashMap<String, String>,
) {
    match node.kind() {
        SyntaxKind::Root | SyntaxKind::Object | SyntaxKind::Value => {
            for child in node.children() {
                collect_flat(&child, prefix, path, result);
            }
        }
        SyntaxKind::Member => {
            let mut key: Option<String> = None;
            let mut value_node: Option<RedNode> = None;

            for child in node.children() {
                if child.kind() == SyntaxKind::Key {
                    key = util::extract_key_from_node(&child);
                } else if child.kind() != SyntaxKind::Comma {
                    value_node = Some(child);
                }
            }

            if let Some(k) = key {
                path.push(k);
                if let Some(vn) = value_node {
                    match vn.kind() {
                        SyntaxKind::Object => {
                            for child in vn.children() {
                                collect_flat(&child, prefix, path, result);
                            }
                        }
                        SyntaxKind::Array => {
                            let mut idx = 0usize;
                            for child in vn.children() {
                                if child.kind() == SyntaxKind::Value {
                                    path.push(idx.to_string());
                                    collect_flat(&child, prefix, path, result);
                                    path.pop();
                                    idx += 1;
                                } else if child.kind() == SyntaxKind::String
                                    || child.kind() == SyntaxKind::BlockString
                                    || child.kind() == SyntaxKind::UnquotedString
                                {
                                    path.push(idx.to_string());
                                    if let Some(val) = util::extract_string_from_node(&child) {
                                        let key_str = build_key(prefix, path);
                                        result.insert(key_str, val);
                                    }
                                    path.pop();
                                    idx += 1;
                                }
                            }
                        }
                        _ => {
                            if let Some(val) = util::extract_string_from_node(&vn) {
                                let key_str = build_key(prefix, path);
                                result.insert(key_str, val);
                            } else {
                                for child in vn.children() {
                                    collect_flat(&child, prefix, path, result);
                                }
                            }
                        }
                    }
                }
                path.pop();
            }
        }
        SyntaxKind::String | SyntaxKind::BlockString | SyntaxKind::UnquotedString => {
            if let Some(val) = util::extract_string_from_node(node) {
                let key_str = build_key(prefix, path);
                result.insert(key_str, val);
            }
        }
        _ => {
            for child in node.children() {
                collect_flat(&child, prefix, path, result);
            }
        }
    }
}

fn build_key(prefix: &str, path: &[String]) -> String {
    if prefix.is_empty() {
        path.join(".")
    } else {
        format!("{}.{}", prefix, path.join("."))
    }
}

pub(crate) mod util {
    use hjson_edit::ast::QuoteStyle;
    use hjson_edit::{RedNode, SyntaxKind};

    pub fn extract_string_from_node(node: &RedNode) -> Option<String> {
        let token = node.first_token()?;
        let text = token.text();

        let quote_style = match token.kind() {
            SyntaxKind::String => QuoteStyle::DoubleQuoted,
            SyntaxKind::BlockString => QuoteStyle::Block,
            SyntaxKind::UnquotedString => QuoteStyle::Unquoted,
            _ => return None,
        };

        Some(quote_style.decode_body(text))
    }

    /// HJSON ではキーは常に文字列扱いのため、パーサがキーとして許容する
    /// 数値・真偽値・null トークンもそのまま文字列として返す。
    pub fn extract_key_from_node(node: &RedNode) -> Option<String> {
        let token = node.first_token()?;
        let text = token.text();

        let quote_style = match token.kind() {
            SyntaxKind::String => QuoteStyle::DoubleQuoted,
            SyntaxKind::BlockString => QuoteStyle::Block,
            SyntaxKind::UnquotedString => QuoteStyle::Unquoted,
            SyntaxKind::Integer
            | SyntaxKind::Float
            | SyntaxKind::True
            | SyntaxKind::False
            | SyntaxKind::Null => return Some(text.to_string()),
            _ => return None,
        };

        Some(quote_style.decode_body(text))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flatten_includes_numeric_keyed_members() {
        let loc = LocalizationFile::parse_with(
            "Localization/en-US_Mods.Example.hjson",
            r#"{ Chat: { 0: "Oink!", 1: "Squeal..." } }"#,
            KNOWN_CULTURES,
        )
        .unwrap();
        let flat = loc.flatten().unwrap();
        assert_eq!(
            flat.get("Mods.Example.Chat.0").map(String::as_str),
            Some("Oink!")
        );
        assert_eq!(
            flat.get("Mods.Example.Chat.1").map(String::as_str),
            Some("Squeal...")
        );
    }

    #[test]
    fn flatten_still_extracts_quoted_keys() {
        let loc = LocalizationFile::parse_with(
            "Localization/en-US_Mods.Example.hjson",
            r#"{ "Quoted Key": "value" }"#,
            KNOWN_CULTURES,
        )
        .unwrap();
        let flat = loc.flatten().unwrap();
        assert_eq!(
            flat.get("Mods.Example.Quoted Key").map(String::as_str),
            Some("value")
        );
    }
}
