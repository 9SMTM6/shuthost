CREATE TABLE host_stats (
    hostname      TEXT NOT NULL PRIMARY KEY,
    last_online   DATETIME NOT NULL,
    agent_version TEXT
);
