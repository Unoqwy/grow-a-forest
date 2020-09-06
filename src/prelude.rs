use std::collections::HashMap;
use std::hash::Hash;
use std::time::Instant;

use futures::future::BoxFuture;
use serenity::prelude::TypeMapKey;
use sqlx::postgres::PgPool;

use serenity::model::channel::ReactionType;

pub use crate::models::*;

macro_rules! eformat {
    ($lit:expr) => {
        format!(gaf_macros::full_format($lit))
    };
    ($lit:expr, $($arg:tt)*) => {
        format!(gaf_macros::full_format!($lit), $($arg)*)
    };
}

macro_rules! quick_init {
    (
        $ctx:ident -> $data:ident $(~$to_drop:ident~)*
        $(=> $(p:$pool:ident)? $(sc:$sc:ident)?)?
        $([<- $(bi:$bi:ident)?])?
        $(; $msg:ident => $(s:$server:ident $([$player:ident])?)?)?
    ) => {
        $( std::mem::drop($to_drop); )*
        let $data = $ctx.data.read().await;
        $(
            $( let $pool = $data.get::<crate::prelude::DatabaseConnection>().unwrap(); )?
            $( let $sc = $data.get::<crate::prelude::ServerCache>().unwrap(); )?
        )?
        $(
            $( let $bi = $ctx.cache.current_user_id().await; )?
        )?
        $(
            $( 
                let $server = $data.get::<crate::prelude::ServerCache>().unwrap().get(&$msg.guild_id.unwrap().0).unwrap();
                $( let $player = $server.player_cache.get(&$msg.author.id.0).unwrap(); )?
            )?
        )?
    };
    (
        $ctx:ident $(~$to_drop:ident~)*
        $(=> $(p:$pool:ident)? $(sc:$sc:ident)?)?
        $([<- $(bi:$bi:ident)?])?
        $(; $msg:ident => $(s:$server:ident $([$player:ident])?)?)?
    ) => {
        quick_init!(
            $ctx -> _data $(~$to_drop~)*
            $(
                => $(p:$pool)? $(sc:$sc)?
            )?
            $(
                [<- $(bi:$bi)?]
            )?
            $(
                ; $msg => $(s:$server $([$player])?)? 
            )?
        );
    };
    (
        $ctx:ident -> mut $data:ident $(~$to_drop:ident~)*
        $(=> $(sc:$sc:ident)? $(pc:$pc:ident)?)?
        $(; $msg:ident => $(s:$server:ident $([$player:ident])?)?)?
    ) => {
        $( std::mem::drop($to_drop); )*
        let mut $data = $ctx.data.write().await;
        $(
            $( let $sc = $data.get_mut::<crate::prelude::ServerCache>().unwrap(); )?
            $( let $pc = $data.get_mut::<crate::prelude::PlantCooldown>().unwrap(); )?
        )?
        $(
            $( 
                let $server = $data.get_mut::<crate::prelude::ServerCache>().unwrap().0.get_mut(&$msg.guild_id.unwrap().0).unwrap();
                $( let $player = $server.player_cache.1.get_mut(&$msg.author.id.0).unwrap(); )?
            )?
        )?
    };
}

macro_rules! success {
    ($ctx:ident, $channel:expr => $msg:expr, $($arg:tt)*) => ({
        let _ = $channel.send_message(&$ctx.http, |m| {
            m.embed(|e| e
                .title("Success")
                .color(0x4CAF50)
                .description(format!(":white_check_mark: {}", eformat!($msg, $($arg)*)))
            )
        }).await?;
    });
}

macro_rules! info {
    ($ctx:ident, $channel:expr => $msg:expr, $($arg:tt)*) => ({
        let _ = $channel.send_message(&$ctx.http, |m| {
            m.embed(|e| e
                .title("Information")
                .color(0x2196F3)
                .description(format!(":information_source: {}", eformat!($msg, $($arg)*)))
            )
        }).await?;
    });
    ($ctx:ident, $channel:expr => ($prefix:expr) $msg:expr, $($arg:tt)*) => ({
        let _ = $channel.send_message(&$ctx.http, |m| {
            m.embed(|e| e
                .title("Information")
                .color(0x2196F3)
                .description(format!("{} {}", $prefix, eformat!($msg, $($arg)*)))
            )
        }).await?;
    });
}

macro_rules! error {
    ($ctx:ident, $channel:expr => $msg:expr, $($arg:tt)*) => ({
        let _ = $channel.send_message(&$ctx.http, |m| {
            m.embed(|e: &mut CreateEmbed| e
                .title("Error")
                .color(0xFF5722)
                .description(format!(":x: {}", eformat!($msg, $($arg)*)))
            )
        }).await?;
    });
}

pub const DEFAULT_COLOR: i32 = 0x16AD4C;

lazy_static::lazy_static! {
    pub static ref EMOJI_NUMBERS: Vec<ReactionType> = {
        let mut v = Vec::with_capacity(10);
        v.push(ReactionType::Unicode("0⃣".to_string()));
        v.push(ReactionType::Unicode("1⃣".to_string()));
        v.push(ReactionType::Unicode("2⃣".to_string()));
        v.push(ReactionType::Unicode("3⃣".to_string()));
        v.push(ReactionType::Unicode("4⃣".to_string()));
        v.push(ReactionType::Unicode("5⃣".to_string()));
        v.push(ReactionType::Unicode("6⃣".to_string()));
        v.push(ReactionType::Unicode("7⃣".to_string()));
        v.push(ReactionType::Unicode("8⃣".to_string()));
        v.push(ReactionType::Unicode("9⃣".to_string()));
        v
    };
}

pub struct DatabaseConnection;

impl TypeMapKey for DatabaseConnection {
    type Value = PgPool;
}

// servers and players can be cached as they will never be shared (for writing) across shards
pub struct ServerCache;

impl TypeMapKey for ServerCache {
    type Value = OneDatabaseCache<u64, Server>;
}

pub struct PlantCooldown;

impl TypeMapKey for PlantCooldown {
    type Value = HashMap<u64, Instant>;
}

type OneFetchHook<K, V> = for<'fut> fn(_: &'fut PgPool, _: &'fut K) -> BoxFuture<'fut, Option<V>>;
pub struct OneDatabaseCache<K, V>(pub HashMap<K, V>, OneFetchHook<K, V>);

impl<K, V> OneDatabaseCache<K, V> where 
    K: Hash + Eq + Clone
{
    pub fn new(fetch_hook: OneFetchHook<K, V>) -> OneDatabaseCache<K, V> {
        OneDatabaseCache {
            0: HashMap::new(),
            1: fetch_hook,
        }
    }

    pub fn get(&self, key: &K) -> Option<&V> {
        self.0.get(key)
    }

    pub async fn fetch(&self, pool: &PgPool, key: &K) -> Option<V> {
        self.1(pool, key).await
    }

    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        self.0.insert(key, value)
    }
}

type ParentedOneFetchHook<P, K, V> = for<'fut> fn(_: &'fut PgPool, _: &'fut K, _: &'fut P) -> BoxFuture<'fut, Option<V>>;
pub struct ParentedOneDatabaseCache<P, K, V>(pub P, pub HashMap<K, V>, ParentedOneFetchHook<P, K, V>);

impl<P, K, V> ParentedOneDatabaseCache<P, K, V> where 
    K: Hash + Eq + Clone
{
    pub fn new(parent: P, fetch_hook: ParentedOneFetchHook<P, K, V>) -> ParentedOneDatabaseCache<P, K, V> {
        ParentedOneDatabaseCache {
            0: parent,
            1: HashMap::new(),
            2: fetch_hook,
        }
    }

    pub fn get(&self, key: &K) -> Option<&V> {
        self.1.get(key)
    }

    pub async fn fetch(&self, pool: &PgPool, key: &K) -> Option<V> {
        self.2(pool, key, &self.0).await
    }

    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        self.1.insert(key, value)
    }
}
