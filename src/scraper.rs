use futures::future::BoxFuture;
use poise::serenity_prelude::*;

use crate::Data;
use crate::hashing::compute_dhash;

async fn process_image(data: &Data, artist_id: Option<i64>, url: &str) -> anyhow::Result<()> {
    let response = data
        .booru
        .download(url)
        .await
        .inspect_err(|e| tracing::error!(err = ?e, "an error occurred when fetching image"))?;

    let image = image::load_from_memory(&response)
        .inspect_err(
            |e| tracing::error!(err = ?e, url = url, "an error occurred when loading image"),
        )?
        .to_rgb8();

    let dhash = compute_dhash(&image);

    sqlx::query!(
        r#"
            INSERT INTO
                artist_artworks (artist_id, original_url, dhash)
            VALUES
                ($1, $2, $3)
            ON CONFLICT (original_url) DO NOTHING;
        "#,
        artist_id,
        &url,
        &dhash,
    )
    .execute(&data.db)
    .await
    .inspect_err(
        |e| tracing::error!(err = ?e, "an error occurred when inserting artist to database"),
    )?;

    Ok(())
}

async fn scrape_gelbooru(
    data: &Data,
    artist_id: Option<i64>,
    search_term: &str,
) -> anyhow::Result<()> {
    let mut success_count = 0;
    let mut failure_count = 0;

    let mut offset = 0;
    let limit = 100;

    tracing::info!(search_term = %search_term, "starting gelbooru scrape for tag");

    loop {
        let resp = data
            .booru
            .get_gelbooru(search_term, offset, limit)
            .await
            .inspect_err(
                |e| tracing::error!(err = ?e, "an error occurred when running scrape task"),
            )?;

        if let Some(posts) = resp.posts {
            for chunk in posts.chunks(5) {
                let tasks: Vec<_> = chunk
                    .iter()
                    .map(|post| process_image(data, artist_id, &post.file_url))
                    .collect();

                let results = futures::future::join_all(tasks).await;

                for result in results {
                    if result.is_ok() {
                        success_count += 1;
                    } else {
                        failure_count += 1;
                    }
                }
            }
        }

        if offset + limit >= resp.attributes.count {
            break;
        }

        offset += limit;

        tracing::info!(search_term = %search_term, offset = offset, "scraping gelbooru in progress");
    }

    tracing::info!(
        search_term = %search_term,
        success_count = success_count,
        failure_count = failure_count,
        "finished scraping gelbooru"
    );

    Ok(())
}

async fn scrape_danbooru(
    data: &Data,
    artist_id: Option<i64>,
    search_term: &str,
) -> anyhow::Result<()> {
    let mut success_count = 0;
    let mut failure_count = 0;

    let mut page = 1;
    let limit = 100;

    tracing::info!(search_term = %search_term, "starting danbooru scrape for tag");

    loop {
        let resp = data
            .booru
            .get_danbooru(search_term, limit, page)
            .await
            .inspect_err(
                |e| tracing::error!(err = ?e, "an error occurred when running scrape task"),
            )?;

        for chunk in resp.chunks(5) {
            let tasks: Vec<_> = chunk
                .iter()
                .filter(|post| post.file_url.is_some())
                .map(|post| {
                    let url = post.file_url.as_ref().unwrap();

                    process_image(data, artist_id, url)
                })
                .collect();

            let results = futures::future::join_all(tasks).await;

            for result in results {
                if result.is_ok() {
                    success_count += 1;
                } else {
                    failure_count += 1;
                }
            }
        }

        if resp.is_empty() || resp.len() < limit as usize {
            break;
        }

        page += 1;

        tracing::info!(search_term = %search_term, page = page, "scraping danbooru in progress");
    }

    tracing::info!(
        search_term = %search_term,
        success_count = success_count,
        failure_count = failure_count,
        "finished scraping gelbooru"
    );

    Ok(())
}

pub async fn scrape_task(data: &Data) -> anyhow::Result<()> {
    let db_artists: Vec<_> = sqlx::query!(
        r#"
            SELECT a.id as artist_id, a.name as search_term
            FROM ai_artists a
            UNION ALL
            SELECT al.artist_id, al.alias as search_term
            FROM ai_artist_aliases al;
        "#,
    )
    .fetch_all(&data.db)
    .await
    .inspect_err(|e| tracing::error!(err = ?e, "an error occurred when fetching artists"))?
    .into_iter()
    .filter_map(|rec| rec.search_term.map(|term| (rec.artist_id, term)))
    .collect();

    let search_terms: Vec<String> = db_artists.iter().map(|(_, term)| term.clone()).collect();
    tracing::info!("list of current artists: {}", search_terms.join(", "));

    for (artist_id, search_term) in db_artists {
        let tasks: Vec<BoxFuture<'_, anyhow::Result<_>>> = vec![
            Box::pin(async { scrape_gelbooru(data, artist_id, &search_term).await }),
            Box::pin(async { scrape_danbooru(data, artist_id, &search_term).await }),
        ];

        let _ = futures::future::join_all(tasks).await;
    }

    Ok(())
}
