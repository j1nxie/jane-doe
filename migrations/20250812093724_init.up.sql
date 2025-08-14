-- Add up migration script here

CREATE TABLE ai_artists (
    id BIGSERIAL PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    created_at TIMESTAMPTZ DEFAULT NOW(),

    UNIQUE(name)
);

CREATE TABLE ai_artist_aliases (
    id BIGSERIAL PRIMARY KEY,
    artist_id BIGINT REFERENCES ai_artists(id) ON DELETE CASCADE,
    alias VARCHAR(255) NOT NULL,
    created_at TIMESTAMPTZ DEFAULT NOW(),

    UNIQUE(artist_id, alias)
);

CREATE TABLE artist_artworks (
    id BIGSERIAL PRIMARY KEY,
    artist_id BIGINT REFERENCES ai_artists(id) ON DELETE CASCADE,
    original_url TEXT NOT NULL,
    dhash BYTEA NOT NULL,
    scraped_at TIMESTAMPTZ DEFAULT NOW(),

    UNIQUE(original_url)
);

CREATE TABLE scrape_status (
    id INTEGER PRIMARY KEY,
    artist_id BIGINT REFERENCES ai_artists(id) ON DELETE CASCADE,
    latest_post_id BIGINT NOT NULL,
    platform TEXT NOT NULL,
    scraped_at TIMESTAMPTZ,

    UNIQUE(artist_id, platform)
);

CREATE INDEX idx_dhash ON artist_artworks(dhash);
CREATE INDEX idx_dhash_prefix ON artist_artworks(substring(dhash FROM 1 FOR 32));
CREATE INDEX idx_artist_alias ON ai_artist_aliases(artist_id, alias);

WITH initial_seeds AS (
    INSERT INTO ai_artists (name)
    VALUES
        ('unfairr'),
        ('setsumanga'),
        ('eroticnansensu')
    ON CONFLICT (name) DO NOTHING
    RETURNING id, name
)
INSERT INTO ai_artist_aliases (artist_id, alias)
SELECT
    aa.id, alias_data.alias
FROM initial_seeds aa
CROSS JOIN LATERAL (
    VALUES
        (CASE WHEN aa.name = 'setsumanga' THEN 'setsuaiart' END),
        (CASE WHEN aa.name = 'eroticnansensu' THEN 'erotic_nansensu' END)
) AS alias_data(alias)
WHERE alias_data.alias IS NOT NULL
ON CONFLICT (artist_id, alias) DO NOTHING;
