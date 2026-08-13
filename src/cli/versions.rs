use tml_workshop::WorkshopClient;

use super::CliError;
use super::args::VersionsArgs;

pub async fn run(args: VersionsArgs) -> Result<(), CliError> {
    let client = WorkshopClient::connect().await?;
    let item_id = super::resolve_item(&client, &args.item).await?;
    let versions = client.list_versions(item_id, args.limit).await?;

    for version in versions {
        println!("{version}");
    }

    Ok(())
}
