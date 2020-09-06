CREATE TABLE IF NOT EXISTS servers (
    id Bigint PRIMARY KEY
  , prefix Varchar NOT NULL DEFAULT '-f'
  , lang Varchar NOT NULL DEFAULT 'en'
);
