use std::time::Duration;

use serenity::{
    prelude::*,
    model::prelude::*,
    framework::standard::{
        macros::command,
        CommandResult, Args
    },
    builder::CreateEmbed,
    utils::Colour
};

use crate::prelude::*;

macro_rules! storage_field {
    (($e:ident) $title:expr, $storage:expr, $item_type:expr, $map:tt) => {
        let mut _lines = $storage.iter()
            .filter(|((item_type, _), amt)| *item_type == $item_type && amt != &&0)
            .map(|((_, k), amount)| {
                (*k, $map(k, if *amount == -1 {
                    "∞".to_string()
                } else {
                    amount.to_string()
                }))
            })
            .collect::<Vec<(i16, String)>>();
        if !_lines.is_empty() {
            _lines.sort_by(|(a, _), (b, _)| a.cmp(b));
            $e.field($title, _lines.iter()
                .map(|(_, s)| s.clone())
                .collect::<Vec<String>>()
                .join("\n"), true);
        }
    };
}

#[command("storage")]
#[aliases("shed", "seedlings")]
async fn cmd_storage(ctx: &Context, message: &Message) -> CommandResult {
    quick_init!(ctx; message => s:server [player]);
    let _ = message.channel_id.send_message(&ctx.http, |m| {
        m.embed(|e: &mut CreateEmbed| {
            e.title("Your storage");
            e.color(DEFAULT_COLOR);

            e.description(eformat!(
               "/:shed/ You own a **{storage_size}** shed.
                Storage capacity: **/:pallet/ {max_pallets} pallets** and **:seedling: {max_seedlings} seedlings**",
                storage_size = "small", max_pallets = 50, max_seedlings = 250,
            ));

            storage_field!((e) "Pallets", player.storage, ItemType::Pallet, (|k, amount| {
                if k == &0 {
                    eformat!("/:pallet/ Empty pallets: **{}**", amount)
                } else {
                    let species = server.species.get(k).unwrap();
                    eformat!("/:pallet/{}: **{}**", species.emoji, amount)
                }
            }));
            storage_field!((e) "Seedlings", player.storage, ItemType::Seedling, (|k, amount| {
                let species = server.species.get(k).unwrap();
                format!(":seedling:{}: **{}**", species.emoji, amount)
            }));

            e
        });
        m
    }).await?;

    Ok(())
}

#[command("shop")]
#[aliases("store")]
async fn cmd_shop(ctx: &Context, message: &Message, _args: Args) -> CommandResult {
    quick_init!(ctx -> data =>; message => s:server [player]);
    let mut msg = message.channel_id.say(&ctx.http, "Loading the shop...").await?;

    let buyable_species: Vec<Species> = server.species.values()
        .filter(|s| s.pallet_cost > 0)
        .map(|s| s.clone())
        .collect();
    let player_coins = player.coins;
    std::mem::drop(data);

    for i in 1..=buyable_species.len() {
        let _ = msg.react(&ctx.http, EMOJI_NUMBERS[i].clone()).await?;
    }
    msg.edit(&ctx.http, |m| {
        m.content("");
        m.embed(|e: &mut CreateEmbed| {
            e.title("Shop");
            e.color(DEFAULT_COLOR);
            
            let mut lines = Vec::new();
            for (i, species) in buyable_species.iter().enumerate() {
                lines.push(eformat!(
                    "/:pallet/{emoji} `{identifier}. {name} Pallet` [{cost} /:coin/]",
                    emoji = species.emoji, name = species.name, cost = species.pallet_cost,
                    identifier = i + 1
                ));
            }
            e.description(eformat!("{}\n\nYour balance: **{}** /:coin/", lines.join("\n"), player_coins));
            e
        });
        m
    }).await?;

    if let Some(reaction_action) = msg.await_reaction(&ctx)
        .author_id(message.author.id)
        .filter(|r| EMOJI_NUMBERS.contains(&r.emoji))
        .timeout(Duration::from_secs(60))
        .await {
        if let ReactionType::Unicode(emoji) = &reaction_action.as_inner_ref().emoji {
            let identifier = (emoji.chars().next().unwrap() as u32) - 48 - 1;
            if let Some(species) = buyable_species.get(identifier as usize) {
                let _ = create_shop_transaction(ctx, &message, species).await?;
            }
        }
    }

    Ok(())
}

async fn create_shop_transaction(ctx: &Context, origin: &Message, species: &Species) -> CommandResult {
    let user = &origin.author;
    let mut msg = origin.channel_id.send_message(&ctx.http, |m|
        m.embed(|e| shop_transaction_create_embed(e, "PENDING", 0x303F9F, user, &species))
    ).await?;

    let _ = msg.react(&ctx.http, ReactionType::Unicode("✅".to_string())).await?;
    let _ = msg.react(&ctx.http, ReactionType::Unicode("❌".to_string())).await?;

    if let Some(reaction_action) = msg.await_reaction(&ctx)
        .author_id(user.id)
        .filter(|r| match &r.emoji {
            ReactionType::Unicode(emoji) => emoji == "✅" || emoji == "❌",
            _ => false
        })
        .timeout(Duration::from_secs(45))
        .await {
        if let ReactionType::Unicode(emoji) = &reaction_action.as_inner_ref().emoji {
            if emoji.as_str() == "✅" {
                quick_init!(ctx -> mut data =>; origin => s:server [player]);
                if player.coins >= species.pallet_cost {
                    player.coins -= species.pallet_cost;

                    let (player_id, species_id) = (player.id, species.id);
                    let (cost, qty) = (species.pallet_cost, 1);
                    let _ = player.give_item(ItemType::Pallet, species_id, qty);

                    quick_init!(ctx ~data~ => p:pool);
                    sqlx::query!(
                       "UPDATE players SET coins = coins - $2 WHERE id = $1",
                       player_id, cost 
                    ).execute(pool).await?;
                    sqlx::query!(
                       "INSERT INTO storage (player_id, item_type, item_id, amount) VALUES ($1, $2, $3, $4)
                        ON CONFLICT(player_id, item_type, item_id) DO UPDATE SET amount = storage.amount + $4",
                        player_id, ItemType::Pallet as i16, species_id, qty
                    ).execute(pool).await?;

                    msg.edit(&ctx.http, |m|
                        m.embed(|e| shop_transaction_create_embed(e, "CONFIRMED", 0x03A9F4, user, &species))
                    ).await?;
                } else {
                    msg.edit(&ctx.http, |m|
                        m.embed(|e| shop_transaction_create_embed(e, "CACELLED; NOT ENOUGH COINS", 0xFFA000, user, &species))
                    ).await?;
                }
                return Ok(());
            }
        }
        msg.edit(&ctx.http, |m|
            m.embed(|e| shop_transaction_create_embed(e, "CANCELLED", 0xFFA000, user, &species))
        ).await?;
    } else {
        msg.edit(&ctx.http, |m|
            m.embed(|e| shop_transaction_create_embed(e, "TIMED OUT", 0xFFA000, user, &species))
        ).await?;
    }

    Ok(())
}

fn shop_transaction_create_embed<'a, C>(e: &'a mut CreateEmbed, status: &'a str, color: C, user: &User, species: &Species) 
    -> &'a mut CreateEmbed
where C: Into<Colour> {
    e.title(format!("Shop Transaction ({})", status));
    e.color(color);

    if status == "PENDING" {
        e.description(eformat!("
            **Cost:** {cost} /:coin/
            **Item:** /:pallet/{emoji} `{name} Pallet`

            **React with :white_check_mark: to confirm the transaction.**
        ", cost = species.pallet_cost, emoji = species.emoji, name = species.name));
    } else {
        e.description(eformat!("
            **Cost:** {cost}/:coin/
            **Item:** /:pallet/{emoji} `{name} Pallet`
        ", cost = species.pallet_cost, emoji = species.emoji, name = species.name));
    }
    
    e.footer(|f| {
        f.text(format!("Transaction holder: {}", user.tag()));
        if let Some(icon_url) = user.avatar_url() {
            f.icon_url(icon_url);
        } else {
            f.icon_url(user.default_avatar_url());
        }
        f
    });
    e
}
