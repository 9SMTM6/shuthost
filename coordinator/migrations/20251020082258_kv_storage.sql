-- This creates a key/value store 

-- used at the time of writing to keep generated tokens and cookie secrets across 
-- restarts so operators do not lose access when they did not explicitly configure
-- these values in the TOML file.

CREATE TABLE IF NOT EXISTS kv_store (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
