-- Player items storage
CREATE TABLE storage (
    player_id Int NOT NULL -- Player identifier unique per user and server (see players.id)
  , item_type Smallint NOT NULL -- Item type (pallet, seedlings, or more later)
  , item_id Smallint NOT NULL -- Item identifier depending on the type
  , amount Int NOT NULL -- How much of this item is stored
  , PRIMARY KEY (player_id, item_type, item_id)
);
