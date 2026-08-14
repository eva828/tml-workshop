use std::fmt;

use clap::{ArgGroup, Args, Parser, Subcommand, ValueEnum};

#[derive(Parser, Debug)]
#[command(
    name = "tml-workshop",
    author,
    version,
    about = "Download and inspect tModLoader Steam Workshop items",
    long_about = None
)]
pub struct Cli {
    #[arg(short, long, global = true, help = "Enable verbose logging to stderr")]
    pub verbose: bool,

    #[arg(long, global = true, help = "Disable the global cache")]
    pub no_cache: bool,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Show details for a workshop item
    Info(InfoArgs),
    /// List the mod versions of a workshop item
    Versions(VersionsArgs),
    /// Download a .tmod file from a workshop item
    Download(DownloadArgs),
    /// Dump the localization files of a workshop item's mod
    Localize(LocalizeArgs),
}

#[derive(Debug, Clone)]
pub enum ItemSpec {
    Id(u64),
    Slug(String),
}

impl std::str::FromStr for ItemSpec {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(s.parse::<u64>()
            .map(ItemSpec::Id)
            .unwrap_or_else(|_| ItemSpec::Slug(s.to_string())))
    }
}

impl fmt::Display for ItemSpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ItemSpec::Id(id) => write!(f, "{id}"),
            ItemSpec::Slug(slug) => write!(f, "{slug}"),
        }
    }
}

#[derive(Args, Debug)]
pub struct InfoArgs {
    /// Workshop item ID (numeric) or slug (non-numeric)
    #[arg(value_name = "ITEM")]
    pub item: ItemSpec,
}

#[derive(Args, Debug)]
pub struct VersionsArgs {
    /// Workshop item ID (numeric) or slug (non-numeric)
    #[arg(value_name = "ITEM")]
    pub item: ItemSpec,

    /// Maximum number of versions to list
    #[arg(short, long)]
    pub limit: Option<u32>,
}

#[derive(Args, Debug)]
#[command(group(
    ArgGroup::new("version_spec")
        .args(["version", "manifest_id"])
        .multiple(false)
))]
pub struct DownloadArgs {
    /// Workshop item ID (numeric) or slug (non-numeric)
    #[arg(value_name = "ITEM")]
    pub item: ItemSpec,

    /// Download the .tmod for this mod version
    #[arg(long, group = "version_spec", value_name = "VERSION")]
    pub version: Option<String>,

    /// Download the .tmod for this manifest ID
    #[arg(long, group = "version_spec", value_name = "MANIFEST_ID")]
    pub manifest_id: Option<u64>,

    /// Output path template. Supported placeholders:
    /// {modname}, {modversion}, {itemid}, {manifestid}
    #[arg(
        short,
        long,
        default_value = "./output/{modname}.tmod",
        value_name = "TEMPLATE"
    )]
    pub output: String,

    /// Extract the .tmod contents into the output directory
    #[arg(long)]
    pub extract: bool,
}

#[derive(Args, Debug)]
#[command(group(
    ArgGroup::new("version_spec")
        .args(["version", "manifest_id"])
        .multiple(false)
))]
pub struct LocalizeArgs {
    /// Workshop item ID (numeric) or slug (non-numeric)
    #[arg(value_name = "ITEM")]
    pub item: ItemSpec,

    /// Dump the localization for this mod version
    #[arg(long, group = "version_spec", value_name = "VERSION")]
    pub version: Option<String>,

    /// Dump the localization for this manifest ID
    #[arg(long, group = "version_spec", value_name = "MANIFEST_ID")]
    pub manifest_id: Option<u64>,

    /// Output format
    #[arg(short, long, value_enum, default_value = "hjson")]
    pub format: OutputFormat,

    /// Only include entries for this culture (e.g. ja-JP)
    #[arg(long, default_value = "en-US")]
    pub culture: String,
}

#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Hjson,
    Json,
    Csv,
    Table,
}
