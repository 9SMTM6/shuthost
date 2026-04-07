CREATE TABLE push_subscriptions (
    id         INTEGER PRIMARY KEY NOT NULL,
    endpoint   TEXT    NOT NULL UNIQUE,
    p256dh     TEXT    NOT NULL,
    auth       TEXT    NOT NULL,
    created_at TEXT    NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE push_subscription_host_online (
    subscription_id INTEGER NOT NULL REFERENCES push_subscriptions(id) ON DELETE CASCADE,
    hostname        TEXT    NOT NULL,
    PRIMARY KEY (subscription_id, hostname)
);
