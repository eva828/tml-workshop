use std::fmt::Display;

use tml_workshop::WorkshopClient;

use super::CliError;
use super::args::InfoArgs;

pub async fn run(args: InfoArgs) -> Result<(), CliError> {
    let client = WorkshopClient::connect().await?;
    let item_id = super::resolve_item(&client, &args.item).await?;
    let details = client.fetch_item_details(item_id).await?;
    let latest = client.fetch_latest_manifest_id(item_id).await?;

    println!("Item ID:        {}", details.publishedfileid);
    println!("Title:          {}", details.title);
    println!("Description:    {}", truncated(&details.description));
    println!("Consumer AppID: {}", opt(&details.consumer_appid));
    println!("Creator AppID:  {}", opt(&details.creator_appid));
    println!("Manifest ID:    {}", opt(&details.manifest_id));
    println!("File name:      {}", opt(&details.filename));
    println!("Preview URL:    {}", opt(&details.preview_url));
    println!("Time created:   {}", opt(&details.time_created));
    println!("Time updated:   {}", opt(&details.time_updated));
    println!("Creator:        {}", opt(&details.creator));
    println!("Tags:           {}", details.tags.join(", "));
    println!("Latest Manifest:{latest}");

    Ok(())
}

fn opt<T: Display>(value: &Option<T>) -> String {
    value
        .as_ref()
        .map(|v| v.to_string())
        .unwrap_or_else(|| "-".to_string())
}

fn truncated(value: &Option<String>) -> String {
    const MAX: usize = 200;

    match value {
        Some(text) if text.len() > MAX => {
            let mut end = MAX;
            while !text.is_char_boundary(end) {
                end -= 1;
            }
            format!("{}...", &text[..end])
        }
        Some(text) => text.clone(),
        None => "-".to_string(),
    }
}
