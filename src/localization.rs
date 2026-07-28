use std::collections::HashMap;

use hjson_edit::{HjsonDocument, RedNode, SyntaxKind};

use crate::error::LocalizationError;

const KNOWN_CULTURES: &[&str] = &[
    "en-US", "de-DE", "it-IT", "fr-FR", "zh-Hans", "es-ES", "ru-RU", "pt-BR", "pl-PL",
];

#[derive(Debug, Clone)]
pub struct CulturePrefix {
    pub culture: String,
    pub prefix: String,
}

impl CulturePrefix {
    pub fn parse(path: &str) -> Option<Self> {
        let path = path.strip_suffix(".hjson").unwrap_or(path);
        let path = path.replace('\\', "/");

        for part in path.split('/') {
            for (i, segment) in part.split('_').enumerate() {
                if KNOWN_CULTURES.contains(&segment) {
                    let culture = segment.to_string();
                    let remaining: Vec<&str> = part.split('_').skip(i + 1).collect();
                    let prefix = if remaining.is_empty() {
                        String::new()
                    } else {
                        remaining.join(".")
                    };
                    return Some(CulturePrefix { culture, prefix });
                }
            }
        }

        None
    }
}

pub struct LocalizationFile {
    culture_prefix: CulturePrefix,
    doc: HjsonDocument,
}

impl LocalizationFile {
    pub fn parse(path: &str, content: &str) -> Result<Self, LocalizationError> {
        let culture_prefix = CulturePrefix::parse(path)
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
                    key = util::extract_string_from_node(&child);
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
}
