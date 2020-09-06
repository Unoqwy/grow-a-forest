use serenity::{
    prelude::*,
    model::prelude::*,
    framework::standard::{
        macros::command,
        CommandResult, Args
    },
    builder::CreateEmbed
};
use sqlx::Row;

use crate::prelude::DEFAULT_COLOR;

macro_rules! bake_stats_query {
    ($table:expr, $where:expr, $limit:expr) => {
        sqlx::query(format!("
            WITH total AS (SELECT SUM(count) AS total from {table} WHERE {where})
            SELECT species, SUM(count) as total, 
                ROUND(SUM(count)::float/(SELECT total from total) * 10000) / 100 AS percent
            FROM {table} WHERE {where} GROUP BY species
            ORDER BY total DESC LIMIT {limit}
        ", table = $table, where = $where, limit = $limit).as_str())
    };
    (LEADERBOARDS $table:expr, $where:expr, $limit:expr) => {
        sqlx::query(format!("
            WITH total AS (SELECT SUM(count) AS total from {table} WHERE {where})
            SELECT a.user_id, b.total, 
                ROUND(b.total::float/(SELECT total from total) * 10000) / 100 AS percent,
                a.species AS fav_species, a.channel_id AS fav_channel
            FROM {table} a 
            INNER JOIN (
                SELECT user_id, MAX(count) count, SUM(count) total
                FROM {table} WHERE {where} GROUP BY user_id
            ) b
            ON a.user_id = b.user_id AND a.count = b.count AND {where}
            ORDER BY total DESC LIMIT {limit}
        ", table = $table, where = $where, limit = $limit).as_str())
    };
}

macro_rules! trees_stats {
    (($e:ident) $species_hashmap:expr, $trees_stats:ident) => ({
            let mut trees = Vec::new();
            let mut total_trees = 0;

            for tree in $trees_stats.iter() {
                let total = tree.get::<i64, _>("total");
                total_trees += total;

                let (emoji, name) = if let Some(species) = $species_hashmap.get(&tree.get::<i16, _>("species")) {
                    (species.emoji.clone(), species.name.clone())
                } else {
                    (":heavy_multiplication_x:".to_owned(), "Unknown".to_owned())
                };
                trees.push(format!(
                    "**{}%** ({}) - {} `{}`",
                    tree.get::<f64, _>("percent"), 
                    total, emoji, name
                ));
            }
            $e.field(format!("Trees ({})", total_trees), trees.join("\n"), true);
    });
}

#[command("stats")]
async fn cmd_stats(ctx: &Context, message: &Message, args: Args) -> CommandResult {
    quick_init!(ctx => p:pool; message => s:server);

    let guild_wide = args.current() == Some("server");
    let trees_stats = bake_stats_query!("trees", if guild_wide {"guild_id = $1"} else {"channel_id = $1"}, 5)
        .bind(if guild_wide {server.id} else {message.channel_id.0 as i64})
        .fetch_all(pool).await?;

    let description = if guild_wide {
        let biggest_channel = sqlx::query!(
            "SELECT channel_id, SUM(count) as total FROM trees WHERE guild_id = $1 GROUP BY channel_id ORDER BY SUM(count) DESC LIMIT 1",
            server.id
        ).fetch_one(pool).await?;
        Some(eformat!("/:forest/ Biggest forest: <#{}> ({})", biggest_channel.channel_id, biggest_channel.total.unwrap()))
    } else {
        None
    };

    let _ = message.channel_id.send_message(&ctx.http, |m| {
        m.embed(|e: &mut CreateEmbed| {
            if guild_wide {
                e.title("Server Forest");
            } else {
                e.title("Channel Forest");
            }
            if let Some(desc) = description {
                e.description(desc);
            }
            e.color(DEFAULT_COLOR);

            trees_stats!((e) server.species, trees_stats);
            e
        })
    }).await;
    Ok(())
}

#[command("mystats")]
#[aliases("my-stats")]
async fn cmd_mystats(ctx: &Context, message: &Message, args: Args) -> CommandResult {
    quick_init!(ctx => p:pool; message => s:server);

    let guild_wide = args.current() == Some("server");
    let trees_stats = bake_stats_query!("trees", if guild_wide {
        "guild_id = $1 AND user_id = $2"
    } else {
        "channel_id = $1 AND user_id = $2"
    }, 5)
        .bind(if guild_wide {server.id} else {message.channel_id.0 as i64})
        .bind(message.author.id.0 as i64)
        .fetch_all(pool).await?;

    let _ = message.channel_id.send_message(&ctx.http, |m| {
        m.embed(|e: &mut CreateEmbed| {
            if guild_wide {
                e.title("Personal Forest (Server)");
            } else {
                e.title("Personal Forest (Channel)");
            }
            e.color(DEFAULT_COLOR);

            trees_stats!((e) server.species, trees_stats);
            e
        })
    }).await;
    Ok(())
}

#[command("leaderboard")]
async fn cmd_leaderboard(ctx: &Context, message: &Message, args: Args) -> CommandResult {
    quick_init!(ctx => p:pool; message => s:server);

    let guild_wide = args.current() != Some("channel");
    let leaderboard = bake_stats_query!(LEADERBOARDS "trees", if guild_wide {"guild_id = $1"} else {"channel_id = $1"}, 5)
        .bind(if guild_wide {server.id} else {message.channel_id.0 as i64})
        .fetch_all(pool).await?;

    let _ = message.channel_id.send_message(&ctx.http, |m| {
        m.embed(|e: &mut CreateEmbed| {
            if guild_wide {
                e.title("Server Leaderboard");
            } else {
                e.title("Channel Leaderboard");
            }
            e.color(DEFAULT_COLOR);

            let mut lines = Vec::new();
            for (i, planter) in leaderboard.iter().enumerate() {
                lines.push(eformat!(
                   "{rank} **{percent}%** ({total}) - <@!{user_id}>
                    > Favorite tree: {fav_species}{fav_extra}",
                    rank = match i {
                        0 => ":first_place:".to_owned(),
                        1 => ":second_place:".to_owned(),
                        2 => ":third_place:".to_owned(),
                        _ => format!("#{}", i + 1)
                    },
                    percent = planter.get::<f64, _>("percent"), 
                    total = planter.get::<i64, _>("total"),
                    user_id = planter.get::<i64, _>("user_id"),
                    fav_species = if let Some(fav_species) = server.species.get(&planter.get::<i16, _>("fav_species")) {
                        fav_species.emoji.clone()
                    } else {
                        "*unknown*".to_owned()
                    },
                    fav_extra = if guild_wide {
                        format!(" | Favorite forest: <#{}>", planter.get::<i64, _>("fav_channel"))
                    } else {
                        format!("")
                    }
                ));
            }
            e.field(eformat!("/:ranger/ Best tree planters",), lines.join("\n"), true);
            e
        })
    }).await;

    Ok(())
}
