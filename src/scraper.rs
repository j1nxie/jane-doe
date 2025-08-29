use futures::future::BoxFuture;
use poise::serenity_prelude::*;
use rayon::prelude::*;

use crate::Data;
use crate::hashing::compute_dhash;
use crate::models::database::Platform;

async fn process_image(
    data: &Data,
    artist_id: Option<i64>,
    url: &str,
    id: i64,
    platform: Platform,
) -> anyhow::Result<()> {
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
                artist_artworks (artist_id, platform, original_id, dhash)
            VALUES
                ($1, $2, $3, $4)
            ON CONFLICT (platform, original_id) DO NOTHING;
        "#,
        artist_id,
        platform as Platform,
        id,
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
    let latest_fetched_id = sqlx::query!(
        r#"
            SELECT
                latest_post_id
            FROM
                scrape_status
            WHERE
                artist_id = $1 AND platform = 'gelbooru'
        "#,
        artist_id
    )
    .fetch_optional(&data.db)
    .await
    .inspect_err(|e| tracing::error!(err = ?e, "an error occurred when fetching latest scraped ID from database"))?
    .map_or(0, |rec| rec.latest_post_id);

    let mut success_count = 0;
    let mut failure_count = 0;

    let mut offset = 0;
    let limit = 100;

    let mut curr_highest_post_id = latest_fetched_id;

    tracing::info!(search_term = %search_term, "starting gelbooru scrape for tag");

    loop {
        tracing::info!(search_term = %search_term, offset = offset, "scraping gelbooru in progress");

        let resp = data
            .booru
            .get_gelbooru(search_term, offset, limit)
            .await
            .inspect_err(
                |e| tracing::error!(err = ?e, "an error occurred when running scrape task"),
            )?;

        if let Some(posts) = resp.posts {
            let items: Vec<_> = posts
                .par_iter()
                .filter(|post| i64::from(post.id) > latest_fetched_id)
                .collect();

            if items.is_empty() {
                break;
            }

            for chunk in items.chunks(5) {
                let tasks: Vec<_> = chunk
                    .iter()
                    .map(|post| {
                        process_image(
                            data,
                            artist_id,
                            &post.file_url,
                            post.id.into(),
                            Platform::Gelbooru,
                        )
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

            if let Some(item) = posts.first() {
                curr_highest_post_id = curr_highest_post_id.max(item.id.into());
            }
        }

        if offset + limit >= resp.attributes.count {
            break;
        }

        offset += limit;
    }

    if curr_highest_post_id > latest_fetched_id {
        sqlx::query!(
            r#"
                INSERT INTO
                    scrape_status (artist_id, latest_post_id, platform, scraped_at)
                VALUES
                    ($1, $2, $3, NOW())
                ON CONFLICT (artist_id, platform) DO UPDATE SET
                    latest_post_id = excluded.latest_post_id,
                    scraped_at = NOW();
            "#,
            artist_id,
            curr_highest_post_id,
            Platform::Gelbooru as Platform,
        )
        .execute(&data.db)
        .await
        .inspect_err(|e| tracing::error!(err = ?e, artist_id = artist_id, "an error occurred when inserting latest fetched id"))?;
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
    let latest_fetched_id = sqlx::query!(
        r#"
            SELECT
                latest_post_id
            FROM
                scrape_status
            WHERE
                artist_id = $1 AND platform = 'danbooru'
        "#,
        artist_id
    )
    .fetch_optional(&data.db)
    .await
    .inspect_err(|e| tracing::error!(err = ?e, "an error occurred when fetching latest scraped ID from database"))?
    .map_or(0, |rec| rec.latest_post_id);

    let mut success_count = 0;
    let mut failure_count = 0;

    let mut page = 1;
    let limit = 100;

    let mut curr_highest_post_id = latest_fetched_id;

    tracing::info!(search_term = %search_term, "starting danbooru scrape for tag");

    loop {
        let fetched_page = if latest_fetched_id == 0 {
            page.to_string()
        } else {
            format!("a{curr_highest_post_id}")
        };

        tracing::info!(search_term = %search_term, page = %fetched_page, "scraping danbooru in progress");

        let resp = data
            .booru
            .get_danbooru(search_term, limit, &fetched_page)
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

                    process_image(data, artist_id, url, post.id.into(), Platform::Danbooru)
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

        if let Some(item) = resp.first() {
            curr_highest_post_id = curr_highest_post_id.max(item.id.into());
        }

        if resp.is_empty() || resp.len() < limit as usize {
            break;
        }

        if latest_fetched_id == 0 {
            page += 1;
        }
    }

    if curr_highest_post_id > latest_fetched_id {
        sqlx::query!(
            r#"
                INSERT INTO
                    scrape_status (artist_id, latest_post_id, platform, scraped_at)
                VALUES
                    ($1, $2, $3, NOW())
                ON CONFLICT (artist_id, platform) DO UPDATE SET
                    latest_post_id = excluded.latest_post_id,
                    scraped_at = NOW();
            "#,
            artist_id,
            curr_highest_post_id,
            Platform::Danbooru as Platform,
        )
        .execute(&data.db)
        .await
        .inspect_err(|e| tracing::error!(err = ?e, artist_id = artist_id, "an error occurred when inserting latest fetched id"))?;
    }

    tracing::info!(
        search_term = %search_term,
        success_count = success_count,
        failure_count = failure_count,
        "finished scraping danbooru"
    );

    Ok(())
}

async fn scrape_rule34(
    data: &Data,
    artist_id: Option<i64>,
    search_term: &str,
) -> anyhow::Result<()> {
    let latest_fetched_id = sqlx::query!(
        r#"
            SELECT
                latest_post_id
            FROM
                scrape_status
            WHERE
                artist_id = $1 AND platform = 'rule34'
        "#,
        artist_id
    )
    .fetch_optional(&data.db)
    .await
    .inspect_err(|e| tracing::error!(err = ?e, "an error occurred when fetching latest scraped ID from database"))?
    .map_or(0, |rec| rec.latest_post_id);

    let mut success_count = 0;
    let mut failure_count = 0;

    let mut page = 0;
    let limit = 100;

    tracing::info!(search_term = %search_term, "starting rule34 scrape for tag");

    let mut curr_highest_post_id = latest_fetched_id;

    loop {
        tracing::info!(search_term = %search_term, page = page, "scraping rule34 in progress");

        let resp = data
            .booru
            .get_rule34(search_term, page, limit)
            .await
            .inspect_err(
                |e| tracing::error!(err = ?e, "an error occurred when running scrape task"),
            )?;

        let items: Vec<_> = resp
            .par_iter()
            .filter(|post| i64::from(post.id) > latest_fetched_id)
            .collect();

        for chunk in items.chunks(5) {
            let tasks: Vec<_> = chunk
                .iter()
                .map(|post| {
                    process_image(
                        data,
                        artist_id,
                        &post.file_url,
                        post.id.into(),
                        Platform::Rule34,
                    )
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

        if let Some(item) = resp.first() {
            curr_highest_post_id = curr_highest_post_id.max(item.id.into());
        }

        if resp.len() < limit as usize || items.is_empty() {
            break;
        }

        page += 1;
    }

    if curr_highest_post_id > latest_fetched_id {
        sqlx::query!(
            r#"
                INSERT INTO
                    scrape_status (artist_id, latest_post_id, platform, scraped_at)
                VALUES
                    ($1, $2, $3, NOW())
                ON CONFLICT (artist_id, platform) DO UPDATE SET
                    latest_post_id = excluded.latest_post_id,
                    scraped_at = NOW();
            "#,
            artist_id,
            curr_highest_post_id,
            Platform::Rule34 as Platform,
        )
        .execute(&data.db)
        .await
        .inspect_err(|e| tracing::error!(err = ?e, artist_id = artist_id, "an error occurred when inserting latest fetched id"))?;
    }

    tracing::info!(
        search_term = %search_term,
        success_count = success_count,
        failure_count = failure_count,
        "finished scraping rule34"
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
            Box::pin(async { scrape_rule34(data, artist_id, &search_term).await }),
        ];

        let _ = futures::future::join_all(tasks).await;
    }

    Ok(())
}
