#[derive(Debug, Clone)]
pub struct KeyParts {
    pub segments: Vec<String>,
}

impl KeyParts {
    pub fn parse(full_key: &str) -> Self {
        KeyParts {
            segments: full_key.split('.').map(String::from).collect(),
        }
    }
}
