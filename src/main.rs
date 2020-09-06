#![feature(or_patterns)]

use std::env;
use std::collections::HashMap;
use std::time::Instant;

use lazy_static::*;
use regex::Regex;

use serenity::{
    client::{Client, Context, EventHandler},
    model::{
        channel::{Message, ReactionType},
        gateway::{Ready, Activity},
        id::EmojiId,
    },
    framework::standard::{
        macros::{group, hook},
        StandardFramework,
    },
    http::Http,
    async_trait,
};
use sqlx::Row;
use sqlx::postgres::{PgPoolOptions, PgPool};

use crate::prelude::*;
use crate::models::*;
use crate::commands::prelude::*;

#[macro_use]
pub mod prelude;
pub mod models;
mod commands;

struct Handler;

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, ctx: Context, data: Ready) {
        println!("|READY| Logged in as \"{}\" on {} guilds.", data.user.tag(), data.guilds.len());
        ctx.set_activity(Activity::listening("the wind")).await;
    }
}

#[group]
#[commands(
    cmd_help, cmd_ping, cmd_prefix, cmd_invite, cmd_support,
    cmd_stats, cmd_mystats, cmd_leaderboard,
    cmd_settings,
    cmd_storage, cmd_shop,
)]
struct General;

#[tokio::main]
async fn main() -> Result<(), sqlx::Error> {
    dotenv::dotenv().ok();
    let token = env::var(format!("DISCORD_{}", env::var("TK").unwrap_or("TOKEN".to_owned())))
        .expect("discord token");

    let db_pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&env::var("DATABASE_URL").expect("database connection url")).await?;

    let http = Http::new_with_token(&token);
    let bot_id = match http.get_current_application_info().await {
        Ok(info) => info.id,
        Err(why) => panic!("Could not access application info: {:?}", why),
    };

    let framework = StandardFramework::new()
        .configure(|c| c
            .allow_dm(false)
            .with_whitespace(true)
            .case_insensitivity(true)
            .on_mention(Some(bot_id))
            .dynamic_prefix(dynamic_prefix)
        )
        .before(before_hook)
        .normal_message(normal_message)
        .group(&GENERAL_GROUP);

    let mut client = Client::new(&token)
        .framework(framework)
        .event_handler(Handler)
        .await.expect("Unable to create the client!");

    {
        let mut data = client.data.write().await;
        data.insert::<DatabaseConnection>(db_pool);
        data.insert::<PlantCooldown>(HashMap::new());
        data.insert::<ServerCache>(OneDatabaseCache::new(fetch_server));
    }

    if let Err(why) = client.start().await {
        println!("An error occurred while running the client: {:?}", why);
    }

    Ok(())
}

async fn get_rules(pool: &PgPool, kind: u8, guild_id: &u64) -> Rules {
    let rows = sqlx::query!(
        "SELECT scope, allowance FROM rules WHERE guild_id = $1 AND kind = $2 ORDER BY scope ASC",
        *guild_id as i64, kind as i16 
    ).fetch_all(pool).await.unwrap();

    let mut channels = HashMap::new();
    for channel_rule in rows.iter() {
        if channel_rule.scope == 0 {
            continue;
        }
        channels.insert(channel_rule.scope as u64, channel_rule.allowance);
    }

    Rules {
        global: if !rows.is_empty() && rows[0].scope == 0 {
            rows[0].allowance
        } else {
            true
        },
        channels,
    }
}

macro_rules! temp_species {
    ($species:ident, $id:expr, $emoji:expr, $name:expr, $cost:expr, $qty:expr, $coins:expr) => {
        $species.insert($id, Species {
            id: $id,
            emoji: $emoji.to_owned(),
            name: $name.to_owned(),
            pallet_cost: $cost,
            default_qty: $qty,
            coins: $coins,
        });
    };
}

/// Fetch and parse a player from the database to cache it.
/// If the player is new (not present in the db), it gets inserted with default values.
/// 
/// Note: Should be called only once per player (unique per user and per server) during runtime.
#[hook]
async fn fetch_player(pool: &PgPool, user_id: &u64, guild_id: &u64) -> Option<Player> {
    let mut result = sqlx::query("SELECT * FROM players WHERE user_id = $1 AND guild_id = $2")
        .bind(*user_id as i64)
        .bind(*guild_id as i64)
        .fetch_optional(pool).await.unwrap();
    let new = result.is_none();
    if result.is_none() {
        result = sqlx::query("INSERT INTO players (user_id, guild_id) VALUES ($1, $2) ON CONFLICT DO NOTHING RETURNING *")
            .bind(*user_id as i64)
            .bind(*guild_id as i64)
            .fetch_optional(pool).await.unwrap();
    }

    if let Some(result) = result {
        let player_id: i32 = result.get("id");

        let mut storage = HashMap::new();
        let storage_rows = sqlx::query!(
            "SELECT item_type, item_id, amount FROM storage WHERE player_id = $1",
           player_id 
        ).fetch_all(pool).await.unwrap();
        for storage_item in storage_rows.iter() {
            storage.insert((ItemType::from_i16(storage_item.item_type), storage_item.item_id), storage_item.amount);
        }

        Some(Player::new(player_id, *user_id, *guild_id, result.get("coins"), storage, new))
    } else {
        None
    }
}

/// Fetch and parse a server from the database to cache it.
/// If the server is new (not present in the db), it gets inserted with default values.
/// 
/// Note: Should be called only once per server during runtime.
#[hook]
async fn fetch_server(pool: &PgPool, guild_id: &u64) -> Option<Server> {
    let mut result = sqlx::query("SELECT * FROM servers WHERE id = $1")
        .bind(*guild_id as i64)
        .fetch_optional(pool).await.unwrap();
    if result.is_none() {
        result = sqlx::query("INSERT INTO servers (id) VALUES ($1) ON CONFLICT DO NOTHING RETURNING *")
            .bind(*guild_id as i64)
            .fetch_optional(pool).await.unwrap();
    }

    if let Some(result) = result {
        let forest_rules = get_rules(pool, 1, guild_id).await;
        let commands_rules = get_rules(pool, 2, guild_id).await;

        let mut species = HashMap::new();
        temp_species!(species, 1, "🌲", "Evergreen Tree", 0, -1, 1);
        temp_species!(species, 2, "🌳", "Deciduous Tree", 12, 50, 1);
        temp_species!(species, 3, "🌴", "Palm Tree", 15, 30, 1);
        temp_species!(species, 4, "🌵", "Cactus", 25, 20, 2);
        temp_species!(species, 5, "🎍", "Bamboo", 50, 10, 3);

        let mut species_from_emojis = HashMap::new();
        for species in species.values() {
            species_from_emojis.insert(species.emoji.clone(), species.id);
        }

        Some(Server {
            id: result.get("id"),
            prefix: result.get("prefix"),
            lang: result.get("lang"),
            plant_cooldown: result.get("plant_cooldown"),

            forest_rules,
            commands_rules,

            species,
            species_from_emojis,

            player_cache: ParentedOneDatabaseCache::new(*guild_id as u64, fetch_player),
        })
    } else {
        None
    }
}

lazy_static! {
    static ref EMOJI_REGEX: Regex = Regex::new(r"^(?:(\p{Emoji_Presentation}|:\w+:|<:\w+:(\d{17,18})>) *)+$").unwrap();
}

#[hook]
async fn dynamic_prefix(ctx: &Context, message: &Message) -> Option<String> {
    if message.guild_id.is_none() {
        return None;
    }

    let data = ctx.data.read().await;
    let server_cache = data.get::<ServerCache>().unwrap();

    let server_id = message.guild_id.unwrap().0;
    if let Some(server) = server_cache.get(&server_id) {
        return Some(server.prefix.clone());
    }

    let pool = data.get::<DatabaseConnection>().unwrap();
    if let Some(server) = server_cache.fetch(pool, &server_id).await {
        quick_init!(ctx -> mut data ~data~ => sc:server_cache);
        server_cache.insert(server_id, server);
        return Some(server_cache.get(&server_id)?.prefix.clone());
    }
    None
}

macro_rules! create_player {
    ($ctx:ident, $data:ident, $server:ident, $message:ident $(, $end:tt)?) => {
        if !$server.player_cache.1.contains_key(&$message.author.id.0) {
            let user_id = $message.author.id.0;
            let pool = $data.get::<DatabaseConnection>().unwrap();
            if let Some(mut player) = $server.player_cache.fetch(pool, &user_id).await {
                if player.is_new() {
                    let server = $data.get::<ServerCache>().unwrap().get(&$message.guild_id.unwrap().0).unwrap();
                    for species in server.species.values() {
                        if species.default_qty == -1 || species.default_qty > 0 {
                            player.give_item(ItemType::Seedling, species.id, species.default_qty);
                            sqlx::query!(
                            "INSERT INTO storage (player_id, item_type, item_id, amount) VALUES ($1, $2, $3, $4)
                                ON CONFLICT(player_id, item_type, item_id) DO UPDATE SET amount = storage.amount + $4",
                                player.id, ItemType::Seedling as i16, species.id, species.default_qty
                            ).execute(pool).await.unwrap();
                        }
                    }
                }

                quick_init!($ctx -> mut data ~$data~; $message => s:server);
                server.player_cache.insert(user_id, player);
            } else {
                $( $end )?
            }
        }
        else {
            std::mem::drop($data);
        }
    };
}

#[hook]
async fn before_hook(ctx: &Context, message: &Message, command: &str) -> bool {
    quick_init!(ctx -> data; message => s:server);
    if !server.commands_rules.check(&message.channel_id.0) {
        if command == "settings" && (message.member(&ctx.cache).await.unwrap()
            .permissions(&ctx.cache).await.unwrap().manage_guild() || message.author.id.0 == 345259637513256960)
        {
            let _ = message.channel_id.say(&ctx.http, ":warning: *Commands are disabled in this channel but you are bypassing this rule as you have the required permission and trying to use an important command.*").await.unwrap();
            return true;
        }
        return false;
    }

    create_player!(ctx, data, server, message, { 
        return false;
    });
    true
}

#[hook]
async fn normal_message(ctx: &Context, message: &Message) {
    if message.author.bot {
        return;
    }

    if let Some(captures) = EMOJI_REGEX.captures(message.content.as_str()) {
        let emoji = if let Some(custom_emoji_id) = captures.get(2) {
            format!("{}>", custom_emoji_id.as_str())
        } else { 
            captures[1].to_owned()
        };

        quick_init!(ctx -> data; message => s:server);
        if !server.forest_rules.check(&message.channel_id.0) {
            return;
        }

        if let Some(species) = server.species_from_emojis.get(&emoji) {
            let species_id = *species;
            std::mem::drop(species);

            let plant_cooldown = server.plant_cooldown;
            create_player!(ctx, data, server, message);

            let user_id = message.author.id.0;
            if plant_cooldown > 0 {
                quick_init!(ctx -> mut data => pc:pc);
                
                let now = Instant::now();
                if match pc.get(&user_id) {
                    Some(time) => {
                        let elapsed = (now - *time).as_secs();
                        elapsed <= plant_cooldown as u64
                    }
                    None => false
                } {
                    return;
                }
                pc.insert(user_id.clone(), now);
            }

            quick_init!(ctx -> mut data; message => s:server [player]);
            if let Some(amt) = player.storage.get_mut(&(ItemType::Seedling, species_id)) {
                let amount = *amt;
                if amount == -1 || amount > 0 {
                    if amount != -1 {
                        *amt -= 1;
                    }

                    let player_id = player.id;
                    let coins = server.species.get(&species_id).unwrap().coins;
                    player.coins += coins;

                    quick_init!(ctx ~data~ => p:pool);
                    sqlx::query!("
                        UPDATE storage SET amount = amount - 1 
                        WHERE player_id = $1 AND item_type = $2 AND item_id = $3 AND amount > 0;",
                        player_id, ItemType::Seedling as i16, species_id
                    ).execute(pool).await.unwrap();
                    sqlx::query!("
                        INSERT INTO trees (species, user_id, channel_id, guild_id) VALUES ($1, $2, $3, $4) 
                        ON CONFLICT(species, user_id, channel_id) DO UPDATE SET count = trees.count + 1",
                        species_id, user_id as i64, message.channel_id.0 as i64, message.guild_id.unwrap().0 as i64, 
                    ).execute(pool).await.unwrap();
                    if coins > 0 {
                        sqlx::query!(
                            "UPDATE players SET coins = coins + $2 WHERE id = $1",
                            player_id, coins
                        ).execute(pool).await.unwrap();
                    }

                    let _ = message.react(&ctx.http, ReactionType::Unicode("🌱".to_string())).await;
                    return;
                }
            }
            let _ = message.react(&ctx.http, ReactionType::Custom {
                animated: false,
                id: EmojiId(750012121475186760),
                name: Some("missing_seedling".to_owned()),
            }).await;
        }
    }
}
