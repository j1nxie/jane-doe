use reqwest::header::{self, HeaderValue};

use crate::models::danbooru::DanbooruPost;
use crate::models::gelbooru::GelbooruResponse;

#[derive(Debug, Clone)]
pub struct BooruClient {
    client: reqwest::Client,
    gelbooru_config: GelbooruConfig,
    danbooru_config: DanbooruConfig,
}

impl BooruClient {
    pub fn new() -> anyhow::Result<Self> {
        Ok(Self {
            client: reqwest::Client::new(),
            gelbooru_config: GelbooruConfig::try_from_env()?,
            danbooru_config: DanbooruConfig::try_from_env()?,
        })
    }

    pub async fn get_gelbooru(
        &self,
        artist: &str,
        offset: u64,
        limit: u64,
    ) -> anyhow::Result<GelbooruResponse> {
        let response = self
            .client
            .get("https://gelbooru.com/index.php")
            .query(&[
                ("page", "dapi"),
                ("s", "post"),
                ("q", "index"),
                ("limit", &limit.to_string()),
                ("offset", &offset.to_string()),
                ("tags", artist),
                ("json", "1"),
                ("api_key", &self.gelbooru_config.api_key),
                ("user_id", &self.gelbooru_config.user_id),
            ])
            .send()
            .await
            .inspect_err(|e| tracing::error!(err = ?e, "an error occurred when fetching posts"))?
            .json::<GelbooruResponse>()
            .await
            .inspect_err(
                |e| tracing::error!(err = ?e, "an error occurred when decoding response body"),
            )?;

        Ok(response)
    }

    pub async fn get_danbooru(
        &self,
        artist: &str,
        limit: u64,
        page: u64,
    ) -> anyhow::Result<Vec<DanbooruPost>> {
        let response = self
            .client
            .get("https://danbooru.donmai.us/posts.json")
            .header(
                header::USER_AGENT,
                HeaderValue::from_static(
                    "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:109.0) Gecko/20100101 \
                     Firefox/114.0",
                ),
            )
            .query(&[
                ("api_key", &self.danbooru_config.api_key),
                ("login", &self.danbooru_config.login),
                ("tags", &artist.to_string()),
                ("page", &page.to_string()),
                ("limit", &limit.to_string()),
            ])
            .send()
            .await
            .inspect_err(|e| tracing::error!(err = ?e, "an error occurred when fetching posts"))?
            .json::<Vec<DanbooruPost>>()
            .await
            .inspect_err(
                |e| tracing::error!(err = ?e, "an error occurred when decoding response body"),
            )?;

        Ok(response)
    }

    pub async fn download(&self, url: &str) -> anyhow::Result<bytes::Bytes> {
        let bytes = self
            .client
            .get(url)
            .header(header::USER_AGENT, HeaderValue::from_static("curl/8.15.0"))
            .send()
            .await
            .inspect_err(|e| tracing::error!(err = ?e, "an error occurred when fetching image"))?
            .bytes()
            .await
            .inspect_err(|e| tracing::error!(err = ?e, "an error occurred when loading bytes"))?;

        Ok(bytes)
    }
}

#[derive(Debug, Clone)]
pub struct GelbooruConfig {
    pub api_key: String,
    pub user_id: String,
}

impl GelbooruConfig {
    fn try_from_env() -> anyhow::Result<Self> {
        let api_key = std::env::var("GELBOORU_API_KEY").inspect_err(
            |e| tracing::error!(err = ?e, "an error occurred when initializing gelbooru api key"),
        )?;
        let user_id = std::env::var("GELBOORU_USER_ID").inspect_err(
            |e| tracing::error!(err = ?e, "an error occurred when initializing gelbooru user id"),
        )?;

        Ok(Self { api_key, user_id })
    }
}

#[derive(Debug, Clone)]
pub struct DanbooruConfig {
    pub api_key: String,
    pub login: String,
}

impl DanbooruConfig {
    fn try_from_env() -> anyhow::Result<Self> {
        let api_key = std::env::var("DANBOORU_API_KEY").inspect_err(
            |e| tracing::error!(err = ?e, "an error occurred when initializing gelbooru api key"),
        )?;
        let login = std::env::var("DANBOORU_LOGIN").inspect_err(
            |e| tracing::error!(err = ?e, "an error occurred when initializing gelbooru user id"),
        )?;

        Ok(Self { api_key, login })
    }
}
