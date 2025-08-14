use std::collections::HashMap;
use std::process::ExitCode;

use clap::Parser;
use poise::{CreateReply, serenity_prelude as serenity};
use rayon::prelude::*;
use sqlx::{Pool, Postgres};

use crate::booru::BooruClient;
use crate::cli::{Cli, Command};
use crate::constants::STARTUP_TIME;
use crate::hashing::{compute_dhash, hamming_distance};
use crate::init::spawn_background_task;
use crate::models::database::ArtworkMatch;
use crate::scraper::scrape_task;

mod booru;
mod cli;
mod commands;
mod constants;
mod hashing;
mod init;
mod models;
mod scraper;

#[derive(Debug, Clone)]
struct Data {
    db: Pool<Postgres>,
    booru: BooruClient,
}

type Error = Box<dyn std::error::Error + Send + Sync>;
type Context<'a> = poise::Context<'a, Data, Error>;

#[tracing::instrument(skip_all)]
async fn event_handler(
    ctx: &serenity::Context,
    event: &serenity::FullEvent,
    _framework: poise::FrameworkContext<'_, Data, Error>,
    data: &Data,
) -> Result<(), Error> {
    if let serenity::FullEvent::Message { new_message } = event {
        if new_message.author.bot || new_message.attachments.is_empty() {
            return Ok(());
        }

        let mut matches: Vec<ArtworkMatch> = vec![];

        for attachment in &new_message.attachments {
            if attachment.dimensions().is_some() {
                let bytes = attachment.download().await?;
                let image = image::load_from_memory(&bytes)?.to_rgb8();

                let dhash = compute_dhash(&image);

                let exact_matches = sqlx::query!(
                    r#"
                        SELECT aa.name as artist_name
                        FROM artist_artworks aw
                        JOIN ai_artists aa
                        ON aw.artist_id = aa.id
                        WHERE aw.dhash = $1;
                    "#,
                    &dhash,
                )
                .fetch_all(&data.db)
                .await
                .inspect_err(|e| tracing::error!(err = ?e, "an error occurred when fetching exact matches from database"))?;

                if let Some(artwork_match) = exact_matches.first() {
                    tracing::info!(attachment_id = %attachment.id, user = %new_message.author.id, "got known AI art");

                    matches.push(ArtworkMatch {
                        artist_name: artwork_match.artist_name.clone(),
                        confidence: 100.0,
                        hash_distance: 0,
                    });

                    continue;
                }

                let artworks = sqlx::query!(
                    r#"
                        SELECT aa.name as artist_name, aw.dhash
                        FROM artist_artworks aw
                        JOIN ai_artists aa ON aw.artist_id = aa.id
                    "#,
                )
                .fetch_all(&data.db)
                .await
                .inspect_err(|e| tracing::error!(err = ?e, "an error occurred when fetching known artworks from database"))?;

                if let Some(artwork_match) = artworks.into_par_iter().find_map_any(|artwork| {
                    let distance = hamming_distance(&artwork.dhash, &dhash);

                    if distance < 15 {
                        Some(ArtworkMatch {
                            artist_name: artwork.artist_name,
                            confidence: (64 - distance) as f32 / 64.0 * 100.0,
                            hash_distance: distance,
                        })
                    } else {
                        None
                    }
                }) {
                    matches.push(artwork_match);

                    continue;
                };
            }
        }

        if matches.is_empty() {
            return Ok(());
        }

        let mut reply_str = String::from("This message contains known AI art ");

        if matches.len() == 1 {
            reply_str += &format!(
                "by {}, confidence is {:.2}%.",
                matches[0].artist_name, matches[0].confidence
            );
        } else {
            reply_str += "by:\n";

            for (idx, artwork_match) in matches.iter().enumerate() {
                reply_str += &format!(
                    "{}. {}, confidence is {:.2}%.\n",
                    idx + 1,
                    artwork_match.artist_name,
                    artwork_match.confidence
                );
            }
        }

        new_message
            .reply_ping(&ctx.http, reply_str)
            .await
            .inspect_err(|e| tracing::error!(err = ?e, "an error occurred when sending reply"))?;
    }

    Ok(())
}

fn main() -> ExitCode {
    match inner_main() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            tracing::error!("{err:?}");
            ExitCode::FAILURE
        }
    }
}

#[tokio::main]
async fn inner_main() -> anyhow::Result<()> {
    let _ = dotenvy::dotenv();
    let _ = &*STARTUP_TIME;

    init::init_telemetry()?;

    let cli = Cli::parse();

    tracing::info!("initializing... please wait warmly.");

    let db = init::init_database().await?;
    let booru = BooruClient::new()?;

    let data = Data { db, booru };

    tracing::info!("finished initializing!");

    match cli.command {
        Command::Start => {
            let token = std::env::var("DISCORD_TOKEN").expect("missing DISCORD_TOKEN");

            let mut client = init::init_discord_client(&token, data.clone()).await?;
            spawn_background_task(&data);

            client.start().await?;
        }
        Command::Scrape => {
            scrape_task(&data).await?;
        }
    }

    Ok(())
}
