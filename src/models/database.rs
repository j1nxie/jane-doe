use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct AiArtist {
    pub id: i64,
    pub name: String,
    pub aliases: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ArtistArtwork {
    id: i64,
    artist_id: i64,
    original_url: String,
    dhash: Vec<u8>,
    scraped_at: chrono::NaiveDateTime,
}

#[derive(Debug, Clone)]
pub struct ArtworkMatch {
    pub artist_name: String,
    pub confidence: f32,
    pub hash_distance: u32,
}

#[derive(Debug, Clone, PartialEq, PartialOrd, Deserialize, Serialize, sqlx::Type)]
#[sqlx(type_name = "platform", rename_all = "lowercase")]
pub enum Platform {
    Gelbooru,
    Danbooru,
    Rule34,
}
