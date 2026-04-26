CREATE TABLE push_subscription_host_operation_failed (
    subscription_id INTEGER NOT NULL REFERENCES push_subscriptions(id) ON DELETE CASCADE,
    hostname        TEXT    NOT NULL,
    PRIMARY KEY (subscription_id, hostname)
);
