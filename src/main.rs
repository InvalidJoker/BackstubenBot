mod voice;
mod commands;

use crate::voice::VoiceChannelManager;
use serenity::{
    async_trait,
    model::{
        gateway::Ready
        ,
        voice::VoiceState,
    },
    prelude::*,
};
use std::env;
use crate::commands::slowmode;

struct Data {
}
type Error = Box<dyn std::error::Error + Send + Sync>;
type PoiseContext<'a> = poise::Context<'a, Data, Error>;

struct Handler {
    voice_manager: VoiceChannelManager,
}

impl Handler {
    fn new(category_id: u64) -> Self {
        Self {
            voice_manager: VoiceChannelManager::new(category_id),
        }
    }
}

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, ctx: Context, ready: Ready) {
        println!("Bot is ready! Logged in as {}", ready.user.name);

        if let Err(e) = self.voice_manager.initialize(&ctx).await {
            eprintln!("Failed to initialize voice manager: {}", e);
        }
    }

    async fn voice_state_update(&self, ctx: Context, old: Option<VoiceState>, new: VoiceState) {
        let joined = new.channel_id;
        let left = old.and_then(|o| o.channel_id);

        if joined != left {
            println!("Voice state update - Joined: {:?}, Left: {:?}", joined, left);
        }

        if let Some(channel_id) = joined {
            if let Err(e) = self.voice_manager.check_joined(&ctx, channel_id).await {
                eprintln!("Error checking joined channel: {}", e);
            }
        }

        if let Some(channel_id) = left {
            if Some(channel_id) != joined {
                if let Err(e) = self.voice_manager.check_left(&ctx, channel_id).await {
                    eprintln!("Error checking left channel: {}", e);
                }
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();
    let token = env::var("DISCORD_TOKEN")
        .expect("Expected DISCORD_TOKEN environment variable");

    let category_id: u64 = env::var("CATEGORY_ID")
        .expect("Expected CATEGORY_ID environment variable")
        .parse()
        .expect("CATEGORY_ID must be a valid u64");

    let intents = GatewayIntents::GUILD_VOICE_STATES | GatewayIntents::GUILDS;
    
    let framework = poise::Framework::builder()
        .options(poise::FrameworkOptions {
            commands: vec![slowmode()],
            ..Default::default()
        })
        .setup(|ctx, _ready, framework| {
            Box::pin(async move {
                poise::builtins::register_globally(ctx, &framework.options().commands).await?;
                Ok(Data {})
            })
        })
        .build();

    let mut client = Client::builder(&token, intents)
        .event_handler(Handler::new(category_id))
        .framework(framework)
        .await?;

    println!("Starting Discord voice channel management bot...");
    println!("Managing category: {}", category_id);

    if let Err(e) = client.start().await {
        eprintln!("Client error: {}", e);
    }

    Ok(())
}