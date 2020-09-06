CREATE TABLE IF NOT EXISTS trees (
    species Varchar NOT NULL
  , count Int NOT NULL DEFAULT 1
  , user_id Bigint NOT NULL
  , channel_id Bigint NOT NULL
  , guild_id Bigint NOT NULL
  , PRIMARY KEY (species, user_id, channel_id)
);
