DROP TABLE IF EXISTS push_subscription_host_online;

CREATE TABLE push_subscription_host_unscheduled (
    subscription_id INTEGER NOT NULL
        REFERENCES push_subscriptions(id) ON DELETE CASCADE,
    hostname        TEXT    NOT NULL,
    PRIMARY KEY (subscription_id, hostname)
);
