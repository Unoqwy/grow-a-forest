use serenity::{
    prelude::*,
    model::prelude::*,
    framework::standard::{
        macros::command,
        CommandResult, Args
    },
    builder::CreateEmbed,
};
use crate::prelude::*;

use regex::Regex;

macro_rules! rules_summary {
    ($rules:expr, $what:expr, $allowed:expr, $is_or_are:expr) => ({
        let exceptions = $rules.channels.iter()
            .filter(|(_, v)| **v != $rules.global)
            .map(|(k, _)| format!("<#{}>", k))
            .collect::<Vec<String>>();
        format!(
            "{what} {access}",
            what = $what, access = if $rules.global {
                if !exceptions.is_empty() {
                    format!("{} in all channels except {}", $allowed, exceptions.join(", "))
                }
                else {
                    format!("{} in all channels", $allowed)
                }
            } else {
                if !exceptions.is_empty() {
                    format!("{} restricted to the following channels: {}", $is_or_are, exceptions.join(", "))
                }
                else {
                    format!("{} forbidden in every channel", $is_or_are)
                }
            }
        )
    });
}

#[command("settings")]
#[aliases("config")]
async fn cmd_settings(ctx: &Context, message: &Message, mut args: Args) -> CommandResult {
    let permissions = message.member(&ctx.cache).await.unwrap().permissions(&ctx.cache).await?;
    if !permissions.manage_guild() && message.author.id.0 != 345259637513256960 {
        error!(ctx, message.channel_id => "**Insufficient permissions!**\n
        > You must have the permission `MANAGE_GUILD` or be granted the \"bot master\" permission to use this command.",);
        return Ok(());
    }

    quick_init!(ctx -> mut data; message => s:server);
    match args.current() {
        Some("prefix") => {
            if let Some(prefix) = args.current() {
                server.prefix = prefix.to_string();
                success!(ctx, message.channel_id => "Prefix changed to `{}`", server.prefix);

                quick_init!(ctx ~data~ => p:pool; message => s:server);
                sqlx::query!("UPDATE servers SET prefix = $1 WHERE id = $2", server.prefix, server.id)
                    .execute(pool).await?;
            }
            else {
                info!(ctx, message.channel_id => (":gear:") "Current prefix: `{}`", server.prefix);
            }
        }
        Some("lang" | "language") => {
            info!(ctx, message.channel_id => "The language cannot be changed yet. This feature will be available soon!",);
        }
        Some("cooldown") => {
            args.advance();
            if args.current() == None {
                info!(ctx, message.channel_id => (":hourglass:") 
                    "The cooldown between each tree is set to **{} seconds** per member.", server.plant_cooldown);
                return Ok(());
            }

            if let Ok(cooldown) = args.single::<i16>() {
                if cooldown >= 0 && cooldown <= 28800 {
                    server.plant_cooldown = cooldown;
                    success!(ctx, message.channel_id => "Trees cooldown has been set to **{} seconds**.", server.plant_cooldown);

                    quick_init!(ctx ~data~ => p:pool; message => s:server);
                    sqlx::query!("UPDATE servers SET plant_cooldown = $1 WHERE id = $2", server.plant_cooldown, server.id)
                        .execute(pool).await?;
                    return Ok(());
                }
            }
            error!(ctx, message.channel_id => "Please specify a valid time in seconds between 0 and 28800!",);
        }
        Some("rules" | "rule") => {
            args.advance();
            if let Some(kind) = match args.current() {
                Some("forest" | "grow") => Some(1),
                Some("commands" | "command") => Some(2),
                _ => None
            } {
                args.advance();
                if let Some(allowance) = match args.current() {
                    Some("allow" | "true" | "1") => Some(1 as u8),
                    Some("deny" | "false" | "-1") => Some(0 as u8),
                    Some("inherit" | "0") => Some(2 as u8),
                    _ => None
                } {
                    args.advance();
                    if let Some(scope) = match args.current() {
                        Some("server" | "guild" | "0") => Some(0 as u64),
                        Some(thing) => {
                            let mut result = None;
                            let re = Regex::new("[<#>]").unwrap();
                            if let Ok(channel_id) = re.replace_all(&thing, "").into_owned().parse::<u64>() {
                                if let Ok(channels) = message.guild_id.unwrap().channels(&ctx.http).await {
                                    if channels.contains_key(&ChannelId::from(channel_id)) {
                                        result = Some(channel_id);
                                    }
                                }
                            }
                            result
                        }
                        _ => None
                    } {
                        let rules = match kind {
                            1 => &mut server.forest_rules,
                            2 => &mut server.commands_rules,
                            _ => panic!("Unsupported rules")
                        };

                        if allowance == 2 {
                            if scope == 0 {
                                rules.global = true;
                            }
                            else {
                                rules.channels.remove(&scope);
                            }
                            
                            quick_init!(ctx ~rules~~data~ => p:pool; message => s:server);
                            sqlx::query!(
                                "DELETE FROM rules WHERE guild_id = $1 AND kind = $2 AND scope = $3",
                                server.id, kind, scope as i64
                            ).execute(pool).await?;
                        }
                        else {
                            let allowed = allowance == 1;
                            if scope == 0 {
                                rules.global = allowed;
                            }
                            else {
                                rules.channels.insert(scope.clone(), allowed);
                            }

                            quick_init!(ctx ~rules~~data~ => p:pool; message => s:server);
                            sqlx::query!(
                               "INSERT INTO rules (guild_id, kind, scope, allowance) VALUES ($1, $2, $3, $4)
                                ON CONFLICT (guild_id, kind, scope) DO UPDATE SET allowance = $4",
                                server.id, kind, scope as i64, allowed
                            ).execute(pool).await?;
                        }

                        success!(
                            ctx, message.channel_id => "Rules update: __{}__ has been set to **{}** {}.", 
                            match kind {
                                1 => "forest growth",
                                2 => "commands",
                                _ => panic!("Unsupported kind")
                            }, 
                            match allowance {
                                1 => "allowed",
                                0 => "denied",
                                2 => "inherited",
                                _ => panic!("Unsupported allowance")
                            },
                            match scope {
                                0 => "globally".to_owned(),
                                channel => format!("in channel <#{}>", channel)
                            }
                        );
                    } else {
                        error!(ctx, message.channel_id => "Missing or invalid scope! You can use either `server` or mention a channel.",);
                    }
                }
                else {
                    error!(ctx, message.channel_id => "Missing or invalid allowance! Valid options are: `allow`, `deny`, `inherit`",);
                }
            } else {
                error!(ctx, message.channel_id => "Missing or invalid rule kind! You can use either `forest` or `commands`.",);
            }
        }
        Some(_) => {
            error!(ctx, message.channel_id =>
               "Invalid settings argument!
                Arguments: `prefix`, `lang`, `cooldown`, `rules`
                > Using this command without argument will give you an overview of the settings",
            );
        }
        None => {
            let forest_rules_summary = rules_summary!(server.forest_rules, "The forest", "can grow", "is");
            let commands_rules_summary = rules_summary!(server.commands_rules, "Commands", "are allowed", "are");
            
            let _ = message.channel_id.send_message(&ctx.http, |m| {
                m.embed(|e: &mut CreateEmbed| {
                    e.title("Server Configuration");
                    e.color(DEFAULT_COLOR);

                    e.field("General Settings", eformat!(
                       "Prefix: `{}`
                        Language: English :flag_gb:
                        Cooldown: {} seconds (/:forest/)",
                        server.prefix, server.plant_cooldown
                    ), false);
                    e.field("Access Rules", eformat!(
                       "(/:forest/) {}
                        (:space_invader:) {}",
                        forest_rules_summary, commands_rules_summary
                    ), false);
                    
                    e
                });
                m
            }).await?;
        }
    };

    Ok(())
}
