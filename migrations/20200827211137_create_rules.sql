CREATE TABLE IF NOT EXISTS rules (
    guild_id Bigint NOT NULL
  , kind Smallint NOT NULL
  , scope Bigint NOT NULL
  , allowance Boolean NOT NULL
  , PRIMARY KEY (guild_id, kind, scope)
);
