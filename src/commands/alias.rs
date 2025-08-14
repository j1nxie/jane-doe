use poise::serenity_prelude::*;

use crate::models::database::AiArtist;
use crate::{Context, Error};

#[tracing::instrument(skip_all)]
#[poise::command(
    prefix_command,
    guild_only,
    aliases("aliases"),
    subcommands("add", "list", "delete"),
    subcommand_required
)]
pub async fn alias(_: Context<'_>) -> Result<(), Error> {
    Ok(())
}

#[tracing::instrument(skip_all)]
#[poise::command(prefix_command, guild_only)]
pub async fn add(ctx: Context<'_>, artist: String, alias: String) -> Result<(), Error> {
    let existing_ai_artist = sqlx::query!(
        r#"
            SELECT
                id as "id!"
            FROM
                ai_artists
            WHERE
                name = $1;
        "#,
        artist
    )
    .fetch_optional(&ctx.data().db)
    .await
    .inspect_err(|e| {
        tracing::error!(err = ?e, artist = %artist, "an error occurred when fetching artist");
    })?;

    match existing_ai_artist {
        Some(q) => {
            let result = sqlx::query!(
                r#"
                    INSERT INTO
                        ai_artist_aliases (artist_id, alias)
                    VALUES
                        ($1, $2);
                "#,
                q.id,
                alias,
            )
            .execute(&ctx.data().db)
            .await
            .inspect_err(|e| {
                tracing::error!(err = ?e, artist = %artist, alias = %alias, "an error occurred when adding alias for artist");
            });

            if let Err(e) = result {
                if e.as_database_error().unwrap().is_unique_violation() {
                    ctx.send(
                        poise::CreateReply::default()
                            .content(format!("alias \"{alias}\" already exists.")),
                    )
                    .await
                    .inspect_err(
                        |e| tracing::error!(err = ?e, "an error occurred when sending reply"),
                    )?;
                }

                return Ok(());
            }

            ctx.send(
                poise::CreateReply::default()
                    .content(format!("added alias \"{alias}\" for artist \"{artist}\".")),
            )
            .await
            .inspect_err(|e| tracing::error!(err = ?e, "an error occurred when sending reply"))?;
        }
        None => {
            ctx.send(
                poise::CreateReply::default()
                    .content(format!("artist \"{artist}\" does not exist.")),
            )
            .await
            .inspect_err(|e| tracing::error!(err = ?e, "an error occurred when sending reply"))?;
        }
    }

    Ok(())
}

#[poise::command(prefix_command)]
#[tracing::instrument(skip_all)]
pub async fn list(ctx: Context<'_>) -> Result<(), Error> {
    let msg = ctx
        .send(
            poise::CreateReply::default()
                .reply(true)
                .allowed_mentions(CreateAllowedMentions::new().replied_user(false))
                .content("loading... please watch warmly..."),
        )
        .await
        .inspect_err(|e| tracing::error!(err = ?e, "an error occurred when sending reply"))?;

    let rows = sqlx::query!(
        r#"
            SELECT
                aa.id AS "id!",
                aa.name,
                STRING_AGG(al.alias, ', ') as aliases
            FROM ai_artists aa
            LEFT JOIN ai_artist_aliases al ON aa.id = al.artist_id
            GROUP BY aa.id, aa.name
            ORDER BY aa.id;
        "#,
    )
    .fetch_all(&ctx.data().db)
    .await
    .inspect_err(
        |e| tracing::error!(err = ?e, "an error occurred when fetching artists from database"),
    )?;

    let artists: Vec<AiArtist> = rows
        .into_iter()
        .map(|row| {
            let aliases = row
                .aliases
                .map(|s| s.split(", ").map(|s| s.to_string()).collect())
                .unwrap_or_default();

            AiArtist {
                id: row.id,
                name: row.name,
                aliases,
            }
        })
        .collect();

    let mut pages: Vec<String> = vec![];
    let mut current_page: usize = 0;

    for (page, chunk) in artists.chunks(10).enumerate() {
        let mut ai_artist_list_str = String::new();

        for (idx, ai_artist) in chunk.iter().enumerate() {
            let entry_str = if ai_artist.aliases.is_empty() {
                format!("{}. {}\n", idx + 1 + page * 10, ai_artist.name)
            } else {
                format!(
                    "{}. {} ({})\n",
                    idx + 1 + page * 10,
                    ai_artist.name,
                    ai_artist.aliases.join(", ")
                )
            };

            ai_artist_list_str += &entry_str;
        }

        pages.push(ai_artist_list_str);
    }

    if pages.is_empty() {
        msg.edit(
            ctx,
            poise::CreateReply::default()
                .reply(true)
                .allowed_mentions(CreateAllowedMentions::new().replied_user(false))
                .content("no artists found in database!"),
        )
        .await
        .inspect_err(|e| tracing::error!(err = ?e, "an error occurred when editing message"))?;

        return Ok(());
    }

    let ctx_id = ctx.id();
    let author_id = ctx.author().id;
    let first_id = format!("{}first", ctx_id);
    let last_id = format!("{}last", ctx_id);
    let prev_id = format!("{}prev", ctx_id);
    let next_id = format!("{}next", ctx_id);

    msg.edit(
        ctx,
        poise::CreateReply::default()
            .reply(true)
            .allowed_mentions(CreateAllowedMentions::new().replied_user(false))
            .content("here's your artists list!")
            .embed(
                CreateEmbed::default()
                    .title("list of artists")
                    .description(pages[0].clone())
                    .footer(CreateEmbedFooter::new(format!(
                        "page {}/{}",
                        current_page + 1,
                        pages.len(),
                    ))),
            )
            .components(vec![CreateActionRow::Buttons(vec![
                CreateButton::new(&first_id).emoji('⏮').disabled(true),
                CreateButton::new(&prev_id).emoji('◀').disabled(true),
                CreateButton::new(&next_id)
                    .emoji('▶')
                    .disabled(current_page == pages.len() - 1),
                CreateButton::new(&last_id)
                    .emoji('⏭')
                    .disabled(current_page == pages.len() - 1),
            ])]),
    )
    .await
    .inspect_err(|e| tracing::error!(err = ?e, "an error occurred when editing message"))?;

    while let Some(press) = collector::ComponentInteractionCollector::new(ctx)
        .filter(move |press| press.data.custom_id.starts_with(&ctx_id.to_string()))
        .timeout(std::time::Duration::from_secs(60))
        .await
    {
        if press.user.id != author_id {
            press
                .create_response(
                    ctx,
                    CreateInteractionResponse::Message(
                        CreateInteractionResponseMessage::new()
                            .content("you cannot interact with another user's invoked command!")
                            .ephemeral(true),
                    ),
                )
                .await
                .inspect_err(
                    |e| tracing::error!(err = ?e, "an error occurred when creating response"),
                )?;

            continue;
        }

        if press.data.custom_id == prev_id {
            current_page = current_page.saturating_sub(1);
        } else if press.data.custom_id == next_id {
            current_page += 1;
        } else if press.data.custom_id == first_id {
            current_page = 0;
        } else if press.data.custom_id == last_id {
            current_page = pages.len() - 1;
        } else {
            continue;
        }

        press
            .create_response(
                ctx,
                CreateInteractionResponse::UpdateMessage(
                    CreateInteractionResponseMessage::new()
                        .embed(
                            CreateEmbed::default()
                                .title("list of artists")
                                .description(pages[current_page].clone())
                                .footer(CreateEmbedFooter::new(format!(
                                    "page {}/{}",
                                    current_page + 1,
                                    pages.len(),
                                ))),
                        )
                        .components(vec![CreateActionRow::Buttons(vec![
                            CreateButton::new(&first_id)
                                .emoji('⏮')
                                .disabled(current_page == 0),
                            CreateButton::new(&prev_id)
                                .emoji('◀')
                                .disabled(current_page == 0),
                            CreateButton::new(&next_id)
                                .emoji('▶')
                                .disabled(current_page == pages.len() - 1),
                            CreateButton::new(&last_id)
                                .emoji('⏭')
                                .disabled(current_page == pages.len() - 1),
                        ])]),
                ),
            )
            .await
            .inspect_err(
                |e| tracing::error!(err = ?e, "an error occurred when creating response"),
            )?;
    }

    msg.into_message()
        .await?
        .edit(ctx, EditMessage::default().components(vec![]))
        .await
        .inspect_err(|e| tracing::error!(err = ?e, "an error occurred when editing message"))?;

    Ok(())
}

#[tracing::instrument(skip(ctx))]
#[poise::command(prefix_command)]
pub async fn delete(ctx: Context<'_>, #[rest] name: String) -> Result<(), Error> {
    let existing_artist = sqlx::query!(
        r#"
            SELECT
                id AS "id!",
                name
            FROM ai_artists
            WHERE name = $1;
        "#,
        name
    )
    .fetch_optional(&ctx.data().db)
    .await
    .inspect_err(|e| {
        tracing::error!(err = ?e, name = %name, "an error occurred when fetching artist");
    })?;

    match existing_artist {
        Some(_) => {
            sqlx::query!(
                r#"
                    DELETE FROM ai_artists
                    WHERE name = $1;
                "#,
                name
            )
            .execute(&ctx.data().db)
            .await
            .inspect_err(
                |e| tracing::error!(err = ?e, name = %name, "an error occurred when deleting artist")
            )?;

            ctx.send(poise::CreateReply::default().content(format!("deleted artist \"{name}\".")))
                .await
                .inspect_err(
                    |e| tracing::error!(err = ?e, "an error occurred when sending reply"),
                )?;
        }
        None => {
            ctx.send(
                poise::CreateReply::default().content(format!("artist \"{name}\" does not exist.")),
            )
            .await
            .inspect_err(|e| tracing::error!(err = ?e, "an error occurred when sending reply"))?;
        }
    }

    Ok(())
}

#[tracing::instrument(skip(ctx))]
#[poise::command(prefix_command)]
pub async fn delete_alias(ctx: Context<'_>, #[rest] alias: String) -> Result<(), Error> {
    let existing_alias = sqlx::query!(
        r#"
            SELECT
                id AS "id!"
            FROM ai_artist_aliases 
            WHERE alias = $1;
        "#,
        alias
    )
    .fetch_optional(&ctx.data().db)
    .await
    .inspect_err(|e| {
        tracing::error!(err = ?e, alias = %alias, "an error occurred when fetching alias");
    })?;

    match existing_alias {
        Some(_) => {
            sqlx::query!(
                r#"
                    DELETE FROM ai_artist_aliases
                    WHERE alias = $1;
                "#,
                alias
            )
            .execute(&ctx.data().db)
            .await
            .inspect_err(
                |e| tracing::error!(err = ?e, alias = %alias, "an error occurred when deleting alias")
            )?;

            ctx.send(poise::CreateReply::default().content(format!("deleted alias \"{alias}\".")))
                .await
                .inspect_err(
                    |e| tracing::error!(err = ?e, "an error occurred when sending reply"),
                )?;
        }
        None => {
            ctx.send(
                poise::CreateReply::default().content(format!("alias \"{alias}\" does not exist.")),
            )
            .await
            .inspect_err(|e| tracing::error!(err = ?e, "an error occurred when sending reply"))?;
        }
    }

    Ok(())
}
