use poise::CreateReply;
use serenity::all::{Channel, EditChannel, Mentionable};
use crate::{Error, PoiseContext};

#[poise::command(slash_command, default_member_permissions = "MANAGE_CHANNELS")]
pub async fn slowmode(
    ctx: PoiseContext<'_>,
    #[description = "Duration in seconds (0 to disable)"] duration: u16,
    #[description = "Channel to be updated"] channel: Option<Channel>,
) -> Result<(), Error> {
    let channel_id = channel
        .map(|c| c.id())
        .unwrap_or_else(|| ctx.channel_id());

    let channel = channel_id.to_channel(&ctx.http()).await?;

    let mut guild_channel = channel.guild().ok_or_else(|| {
        poise::serenity_prelude::Error::Other("Channel is not a guild channel")
    })?;

    let duration_clone = duration.to_string();

    guild_channel.edit(
        &ctx.http(),
        EditChannel::default().rate_limit_per_user(duration.clone())
    ).await?;

    ctx.send(CreateReply::default()
        .content(format!(
            "Updated slowmode for channel {} to **{}** seconds.",
            guild_channel.mention(),
            if duration == 0 { "disabled" } else { duration_clone.as_str() }
        ))
        .ephemeral(true)
    ).await?;

    Ok(())
}