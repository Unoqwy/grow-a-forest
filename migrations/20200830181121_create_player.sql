CREATE TABLE players (
    id Serial -- Player id for references in other tables
  , user_id Bigint NOT NULL -- Discord user id
  , guild_id Bigint NOT NULL -- Discord guild id
  , storage_upgrade Smallint NOT NULL DEFAULT 1 -- Size of the storage
  , last_time_check Timestamp NOT NULL DEFAULT CURRENT_TIMESTAMP -- Last time check for time-based actions
  , PRIMARY KEY (user_id, guild_id)
);
