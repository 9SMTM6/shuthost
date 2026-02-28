-- Stores runtime IP/port overrides for hosts whose address differs from the config.
-- Updated when an agent startup broadcast arrives with a new address.
CREATE TABLE host_ip_overrides (
    hostname      TEXT     PRIMARY KEY NOT NULL,
    ip            TEXT     NOT NULL,
    port          INTEGER  NOT NULL
);
