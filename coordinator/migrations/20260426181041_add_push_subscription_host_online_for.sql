CREATE TABLE push_subscription_host_online_for (
    subscription_id INTEGER NOT NULL REFERENCES push_subscriptions(id) ON DELETE CASCADE,
    hostname        TEXT    NOT NULL,
    duration_secs   INTEGER NOT NULL,
    PRIMARY KEY (subscription_id, hostname)
);
