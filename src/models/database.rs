#[derive(Debug, Clone)]
pub struct AiArtist {
    id: i64,
    name: i64,
    created_at: chrono::NaiveDateTime,
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
