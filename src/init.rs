use std::str::FromStr;

use chrono::{Duration, Local};
use poise::serenity_prelude::{self as serenity};
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::{Pool, Postgres};
use tokio::time::{Instant, sleep_until};
use tracing::Instrument;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Registry};

use crate::{Data, commands, event_handler, scraper};

pub async fn init_database() -> anyhow::Result<Pool<Postgres>> {
    let db_url = std::env::var("DATABASE_URL").expect("missing DATABASE_URL");

    tracing::info!("initializing database connection...");

    let opts = PgConnectOptions::from_str(&db_url).expect("invalid DATABASE_URL");

    let db = PgPoolOptions::new()
        .max_connections(20)
        .connect_with(opts)
        .await?;

    tracing::info!("running migrations...");
    sqlx::migrate!("./migrations").run(&db).await?;
    tracing::info!("finished running migrations!");

    Ok(db)
}

pub async fn init_discord_client(token: &str, data: Data) -> anyhow::Result<serenity::Client> {
    let intents =
        serenity::GatewayIntents::non_privileged() | serenity::GatewayIntents::MESSAGE_CONTENT;

    let framework = poise::Framework::builder()
        .options(poise::FrameworkOptions {
            commands: vec![
                commands::status::status(),
            ],
            prefix_options: poise::PrefixFrameworkOptions {
                prefix: Some("j!".into()),
                ..Default::default()
            },
            event_handler: |ctx, event, framework, data| {
                Box::pin(event_handler(ctx, event, framework, data))
            },
            ..Default::default()
        })
        .setup(|ctx, _ready, framework| {
            Box::pin(
                async move {
                    poise::builtins::register_globally(ctx, &framework.options().commands)
                        .await
                        .inspect_err(
                            |e| tracing::error!(err = ?e, "an error occurred when registering commands"),
                        )?;

                    Ok(data)
                }
                .in_current_span(),
            )
        })
        .build();

    let client = serenity::ClientBuilder::new(token, intents)
        .framework(framework)
        .await?;

    Ok(client)
}

pub fn init_telemetry() -> anyhow::Result<()> {
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    Registry::default()
        .with(env_filter)
        .with(tracing_subscriber::fmt::layer())
        .try_init()?;

    Ok(())
}

pub fn spawn_background_task(data: &Data) {
    let data_clone = data.clone();

    tokio::spawn(
        async move {
            tracing::info!("initialized background tasks!");

            loop {
                let now = Local::now();
                let next_midnight = {
                    let today_midnight = now
                        .date_naive()
                        .and_hms_opt(0, 0, 0)
                        .unwrap()
                        .and_local_timezone(Local)
                        .single()
                        .unwrap();

                    if now > today_midnight {
                        today_midnight + Duration::days(1)
                    } else {
                        today_midnight
                    }
                };

                let duration_until_midnight = next_midnight.signed_duration_since(now);

                tracing::info!(
                    "next scraping task scheduled for: {} (local time)",
                    next_midnight.format("%Y-%m-%d %H:%M:%S")
                );

                sleep_until(
                    Instant::now()
                        + tokio::time::Duration::from_secs(
                            duration_until_midnight.num_seconds() as u64
                        ),
                )
                .await;

                let _ = scraper::scrape_task(&data_clone).await;
            }
        }
        .in_current_span(),
    );
}
