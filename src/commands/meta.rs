use serenity::{
    prelude::*,
    model::prelude::*,
    framework::standard::{
        macros::command,
        CommandResult, Args
    },
    builder::CreateEmbed,
};
use crate::prelude::DEFAULT_COLOR;

#[command("help")]
async fn cmd_help(ctx: &Context, message: &Message, args: Args) -> CommandResult {
    let _ = message.channel_id.send_message(&ctx.http, |m| {
        m.embed(|e: &mut CreateEmbed| {
            e.title("Grow a Forest Help");
            e.color(DEFAULT_COLOR);
            
            e.description(eformat!("
                /:forest/ To plant a tree, you only have to send a tree emoji (i.e: :evergreen_tree:) in a channel with forest growth enabled.
                You'll need a seedling to plant a tree, you can check how many seedlings of each you currently have with `f-shed`. Game commands allow you to get new seedlings and store them.
            ",));

            e.field("Game commands", "`storage`, `shop`, `greenhouse`, `workers`", false);
            e.field("Stats commands", "`stats`, `mystats`, `leaderboard`", false);
            e.field("Meta commands", "`ping`, `prefix`, `invite`, `support`", false);

            e
        });
        m
    }).await?;
    Ok(())
}

#[command("ping")]
async fn cmd_ping(ctx: &Context, message: &Message) -> CommandResult {
    let mut msg = message.channel_id.say(&ctx.http, ":evergreen_tree: Your signal successfully passed through the forest!").await?;
    let elapsed_millis = msg.timestamp.timestamp_millis() - message.timestamp.timestamp_millis();
    let content = msg.content.clone();
    msg.edit(&ctx.http, |m| {
        m.content(format!("{} (:signal_strength: {}ms)", content, elapsed_millis))
    }).await?;
    Ok(())
}

#[command("prefix")]
async fn cmd_prefix(ctx: &Context, message: &Message) -> CommandResult {
    quick_init!(ctx [<- bi:bot_id]; message => s:server);
    info!(ctx, message.channel_id => 
       "Prefix on this server: `{prefix}`
        If you are unable to use it or ever forget it you may mention me as an alternative.
        > Example: '{prefix}prefix' or '<@{bot_id}> prefix'",
        prefix = server.prefix, bot_id = bot_id
    );
    Ok(())
}

#[command("invite")]
async fn cmd_invite(ctx: &Context, message: &Message) -> CommandResult {
    let _ = message.author.direct_message(&ctx.http, |m| {
        m.embed(|e: &mut CreateEmbed| e
            .title("Invite Link")
            .color(DEFAULT_COLOR)
            .description("Click [this link](https://discord.com/oauth2/authorize?client_id=747556772545298522&scope=bot&permissions=379968) to add me on your server.")
        )
    }).await?;
    let _ = message.channel_id.say(&ctx.http, "Check your DMs!").await?;
    Ok(())
}

#[command("support")]
async fn cmd_support(ctx: &Context, message: &Message) -> CommandResult {
    let _ = message.author.direct_message(&ctx.http, |m| {
        m.content(":evergreen_tree: Need help with Grow a Forest or want to contribute to the Official Forest? Join our Support Server!\nhttps://discord.gg/ngVTXz9")
    }).await?;
    let _ = message.channel_id.say(&ctx.http, "Check your DMs!").await?;
    Ok(())
}