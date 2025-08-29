-- Add down migration script here

DELETE FROM ai_artists
WHERE name IN (
    'shexyo',
    'oroborus',
    'jemmasoria',
    'prixmal',
    'twistedscarlett60'
);
