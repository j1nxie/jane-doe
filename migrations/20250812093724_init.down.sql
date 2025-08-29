-- Add down migration script here

DROP INDEX idx_dhash;
DROP INDEX idx_artist_alias;

DROP TABLE scrape_status;
DROP TABLE artist_artworks;
DROP TABLE ai_artist_aliases;
DROP TABLE ai_artists;

DROP TYPE PLATFORM;
