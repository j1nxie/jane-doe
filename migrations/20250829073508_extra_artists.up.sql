-- Add up migration script here

WITH initial_seeds AS (
    INSERT INTO ai_artists (name)
    VALUES
        ('shexyo'),
        ('oroborus'),
        ('jemmasoria'),
        ('prixmal'),
        ('twistedscarlett60')
    ON CONFLICT (name) DO NOTHING
    RETURNING id, name
)
INSERT INTO ai_artist_aliases (artist_id, alias)
SELECT
    aa.id, alias_data.alias
FROM initial_seeds aa
CROSS JOIN LATERAL (
    VALUES
        (CASE WHEN aa.name = 'oroborus' THEN 'oroborusart' END)
) AS alias_data(alias)
WHERE alias_data.alias IS NOT NULL
ON CONFLICT (artist_id, alias) DO NOTHING;
