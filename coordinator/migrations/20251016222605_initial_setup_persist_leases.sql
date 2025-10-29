-- Persistent database settings that should be applied once per database.
-- These are applied here in a migration so they become part of the DB file
-- and do not rely on runtime initialization. journal_mode and auto_vacuum
-- modify on-disk database behavior and are appropriate to set via migration.
PRAGMA journal_mode = WAL; -- enable WAL for better concurrency
PRAGMA auto_vacuum = INCREMENTAL; -- enable incremental auto_vacuum

-- Create separate tables for each lease source type
CREATE TABLE web_interface_leases (
    hostname TEXT NOT NULL,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(hostname)
);

CREATE TABLE client_leases (
    hostname TEXT NOT NULL,
    client_id TEXT NOT NULL,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(hostname, client_id)
);

-- Indexes for efficient queries
CREATE INDEX idx_client_leases_client_id ON client_leases(client_id);
CREATE INDEX idx_client_leases_hostname ON client_leases(hostname);

-- Optional view for combined queries
CREATE VIEW leases AS
SELECT hostname, 'web_interface' AS lease_source_type, NULL AS lease_source_value, created_at FROM web_interface_leases
UNION ALL
SELECT hostname, 'client' AS lease_source_type, client_id AS lease_source_value, created_at FROM client_leases;
