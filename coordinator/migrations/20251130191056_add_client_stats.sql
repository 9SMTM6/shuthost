-- Add client statistics table to track usage metrics like last used time

CREATE TABLE client_stats (
    client_id TEXT PRIMARY KEY NOT NULL,
    last_used DATETIME -- Stored in UTC
);
