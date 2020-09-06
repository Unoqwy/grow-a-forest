use std::collections::HashMap;
use std::hash::Hash;

use crate::prelude::ParentedOneDatabaseCache;

/// Per-channel boolean rules.
#[derive(Debug)]
pub struct Rules {
    pub global: bool,
    pub channels: HashMap<u64, bool>,
}

impl Rules {
    pub fn check(&self, channel_id: &u64) -> bool {
        if let Some(flag) = self.channels.get(channel_id) {
            *flag
        }
        else {
            self.global
        }
    }
}

/// A representation of a guild with all its linked data cached
pub struct Server {
    /// Discord guild id, stored as i64 for ease of use in queries
    pub id: i64,
    /// Commands prefix
    pub prefix: String,
    /// Language the bot should use in this server
    /// Note: Only English is supported for now
    pub lang: String,

    /// Channel rules where trees can be planted
    pub forest_rules: Rules,
    /// Cooldown between each tree planting
    /// TODO: channel-based cooldown
    pub plant_cooldown: i16,
    /// Channel rules where commands can be executed
    pub commands_rules: Rules,

    /// All tree species, stored by id
    pub species: HashMap<i16, Species>,
    /// All tree species ids, stored by emoji
    /// Combine result with `species` to get the actual value if needed
    pub species_from_emojis: HashMap<String, i16>,

    pub player_cache: ParentedOneDatabaseCache<u64, u64, Player>,
}

/// A tree species
/// 
/// Note: `id` should be unsigned but is stored as i16 to use in queries without casting
#[derive(Debug, Clone)]
pub struct Species {
    /// Incremental id (SERIAL type in Postgres)
    pub id: i16,
    /// Discord emoji representation
    pub emoji: String,
    /// Displayable name
    pub name: String,

    /// Cost per unit in a pallet
    /// Set to 0 to disable purchase
    pub pallet_cost: i32,
    /// Default quantity of seedlings a player will get by default
    /// Set to -1 to give infinite seedlings
    pub default_qty: i32,
    /// Coins given for each tree planted
    /// Set to 0 to disable (obviously)
    pub coins: i32,
}

pub type Storage = HashMap<(ItemType, i16), i32>;

/// A representation of a server player with all its linked data cached
#[derive(Debug)]
pub struct Player {
    /// Unique identifier (different per user and per server)
    pub id: i32,
    /// Discord user id
    pub user_id: u64,
    /// Discord guild id
    pub guild_id: u64,

    /// Player's wallet
    pub coins: i32,
    /// All items in storage
    /// Mapped by (type, id) and gives the amount
    /// Reminder: -1 = infinity
    pub storage: Storage,

    _newly_created: bool,
}

impl Player {
    pub fn new(id: i32, user_id: u64, guild_id: u64, coins: i32, storage: Storage, new: bool) -> Player {
        Player {
            id,
            user_id,
            guild_id,
            
            coins,
            storage,

            _newly_created: new,
        }
    }

    pub fn is_new(&self) -> bool {
        self._newly_created
    }

    pub fn give_item(&mut self, item_type: ItemType, item_id: i16, qty: i32) -> i32 {
        let new_qty = *self.storage.get(&(item_type, item_id)).unwrap_or(&0) + qty;
        self.storage.insert((item_type, item_id), new_qty);
        new_qty
    }
}

#[derive(Debug, Hash, PartialEq, Eq, Clone, Copy)]
pub enum ItemType {
    Pallet = 1,
    Seedling,
}

impl ItemType {
    pub fn from_i16(value: i16) -> ItemType {
        match value {
            1 => ItemType::Pallet,
            2 => ItemType::Seedling,
            v@_ => panic!("Unknown value: {}", v)            
        }
    }
}
