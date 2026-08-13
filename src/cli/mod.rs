pub mod args;
pub mod download;
pub mod info;
pub mod localize;
pub mod versions;

pub use args::ItemSpec;

use std::fmt;

use tml_workshop::{Error, ItemId, WorkshopClient};

#[derive(Debug)]
pub enum CliError {
    Library(Error),
    Message(String),
}

impl CliError {
    pub fn msg(msg: impl Into<String>) -> Self {
        CliError::Message(msg.into())
    }
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CliError::Library(e) => write!(f, "{e}"),
            CliError::Message(m) => write!(f, "{m}"),
        }
    }
}

impl std::error::Error for CliError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            CliError::Library(e) => Some(e),
            CliError::Message(_) => None,
        }
    }
}

impl From<Error> for CliError {
    fn from(e: Error) -> Self {
        CliError::Library(e)
    }
}

impl From<std::io::Error> for CliError {
    fn from(e: std::io::Error) -> Self {
        CliError::Library(e.into())
    }
}

impl From<serde_json::Error> for CliError {
    fn from(e: serde_json::Error) -> Self {
        CliError::Library(std::io::Error::new(std::io::ErrorKind::InvalidData, e).into())
    }
}

impl From<csv::Error> for CliError {
    fn from(e: csv::Error) -> Self {
        CliError::Library(std::io::Error::new(std::io::ErrorKind::InvalidData, e).into())
    }
}

pub fn init_tracing(verbose: bool) {
    let filter = if verbose { "debug" } else { "warn" };
    let env = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(filter));
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(env)
        .init();
}

pub async fn resolve_item(client: &WorkshopClient, spec: &ItemSpec) -> Result<ItemId, CliError> {
    match spec {
        ItemSpec::Id(id) => Ok(ItemId::new(*id)),
        ItemSpec::Slug(slug) => Ok(client.find_item_by_slug(slug).await?),
    }
}
