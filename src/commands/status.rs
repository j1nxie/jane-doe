use std::time::UNIX_EPOCH;

use poise::serenity_prelude as serenity;

use crate::commands::get_bot_avatar;
use crate::constants::version::get_version;
use crate::constants::{POISE_VERSION, STARTUP_TIME};
use crate::{Context, Error};

/// get the bot's status.
#[tracing::instrument(skip_all)]
#[poise::command(prefix_command)]
pub async fn status(ctx: Context<'_>) -> Result<(), Error> {
    ctx.send(poise::CreateReply::default().embed(
        serenity::CreateEmbed::new()
        .field(
            "about the bot",
            "[Jane Doe](https://github.com/j1nxie/jane-doe) is an AI art detector Discord bot, written by [Rylie](https://github.com/j1nxie), using the [poise](https://github.com/serenity-rs/poise) framework.".to_string(),
            false
        )
        .field("version", get_version(), false)
        .field("rust", format!("[{0}](https://releases.rs/docs/{0})", rustc_version_runtime::version().to_string()), true)
        .field("poise", format!("[{0}](https://docs.rs/crate/poise/{0})", POISE_VERSION), true)
        .field("uptime", format!("<t:{}:R>", STARTUP_TIME.duration_since(UNIX_EPOCH).unwrap().as_secs()), true)
        .thumbnail(get_bot_avatar(ctx))
    ))
    .await
    .inspect_err(|e| tracing::error!(err = ?e, "an error occurred when sending reply"))?;

    Ok(())
}
