use std::collections::HashMap;
use std::sync::Arc;
use serenity::all::{ChannelId, ChannelType, Context, CreateChannel, GuildChannel, Http};
use tokio::sync::RwLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VoiceChannelType {
    Unlimited,
    Two,
    Three,
    Four,
    Five,
}

impl VoiceChannelType {
    pub fn identifier(&self) -> char {
        match self {
            Self::Unlimited => '∞',
            Self::Two => '2',
            Self::Three => '3',
            Self::Four => '4',
            Self::Five => '5',
        }
    }

    pub fn user_limit(&self) -> Option<u32> {
        match self {
            Self::Unlimited => None,
            Self::Two => Some(2),
            Self::Three => Some(3),
            Self::Four => Some(4),
            Self::Five => Some(5),
        }
    }

    pub fn by_identifier(identifier: char) -> Option<Self> {
        match identifier {
            '∞' => Some(Self::Unlimited),
            '2' => Some(Self::Two),
            '3' => Some(Self::Three),
            '4' => Some(Self::Four),
            '5' => Some(Self::Five),
            _ => None,
        }
    }

    pub fn all() -> Vec<Self> {
        vec![Self::Unlimited, Self::Two, Self::Three, Self::Four, Self::Five]
    }
}

pub struct VoiceChannelManager {
    category_id: ChannelId,
    channel_cache: Arc<RwLock<HashMap<VoiceChannelType, Vec<ChannelId>>>>,
}

impl VoiceChannelManager {
    pub fn new(category_id: u64) -> Self {
        Self {
            category_id: ChannelId::new(category_id),
            channel_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn initialize(&self, ctx: &Context) -> Result<(), serenity::Error> {
        println!("Initializing voice channel manager...");

        let mut cache = self.channel_cache.write().await;

        let category = self.category_id.to_channel(&ctx.http).await?
            .guild().ok_or_else(|| serenity::Error::Other("Category not found"))?;

        let guild_id = category.guild_id;
        let guild_channels = guild_id.channels(&ctx.http).await?;

        let channels_in_category: Vec<_> = guild_channels.values()
            .filter(|ch| ch.parent_id == Some(self.category_id))
            .collect();

        println!("Found {} channels in category", channels_in_category.len());

        for channel in channels_in_category {
            if channel.kind != ChannelType::Voice {
                continue;
            }

            if let Some(identifier) = channel.name.chars().last() {
                if let Some(channel_type) = VoiceChannelType::by_identifier(identifier) {
                    cache.entry(channel_type).or_insert_with(Vec::new).push(channel.id);
                    println!("Loaded existing channel: {} ({})", channel.name, channel_type.identifier());
                }
            }
        }

        for channel_type in VoiceChannelType::all() {
            if !cache.contains_key(&channel_type) || cache[&channel_type].is_empty() {
                let new_channel = self.create_channel(&ctx.http, channel_type, guild_id).await?;
                cache.entry(channel_type).or_insert_with(Vec::new).push(new_channel.id);
                println!("Created new channel: {} ({})", new_channel.name, channel_type.identifier());
            }
        }

        drop(cache);
        self.sort_channels(ctx, guild_id).await?;
        println!("Voice channel manager initialized successfully!");
        Ok(())
    }

    async fn create_channel(&self, http: &Http, channel_type: VoiceChannelType, guild_id: serenity::model::id::GuildId) -> Result<GuildChannel, serenity::Error> {
        let builder = CreateChannel::new(format!("🔊voice {}", channel_type.identifier()))
            .kind(ChannelType::Voice)
            .category(self.category_id);

        let builder = if let Some(limit) = channel_type.user_limit() {
            builder.user_limit(limit)
        } else {
            builder
        };

        guild_id.create_channel(http, builder).await
    }

    pub(crate) async fn check_joined(&self, ctx: &Context, channel_id: ChannelId) -> Result<(), serenity::Error> {
        let channel = channel_id.to_channel(&ctx.http).await?
            .guild().ok_or_else(|| serenity::Error::Other("Channel not in guild"))?;

        let identifier = channel.name.chars().last().unwrap_or_default();
        let channel_type = match VoiceChannelType::by_identifier(identifier) {
            Some(t) => t,
            None => return Ok(()),
        };

        let cache = self.channel_cache.read().await;
        let current_channels = cache.get(&channel_type).cloned().unwrap_or_default();
        drop(cache);

        if current_channels.len() >= 6 {
            return Ok(());
        }

        let mut found_empty = false;
        for &ch_id in &current_channels {
            if let Ok(ch) = ch_id.to_channel(&ctx.http).await {
                if let Some(guild_channel) = ch.guild() {
                    let members = guild_channel.members(&ctx.cache);
                    if members.map_or(true, |m| m.is_empty()) {
                        found_empty = true;
                        break;
                    }
                }
            }
        }

        if !found_empty {
            let guild_id = channel.guild_id;
            let new_channel = self.create_channel(&ctx.http, channel_type, guild_id).await?;
            let mut cache = self.channel_cache.write().await;
            cache.entry(channel_type).or_insert_with(Vec::new).push(new_channel.id);
            drop(cache);
            self.sort_channels(ctx, guild_id).await?;
            println!("Created new channel due to full occupancy: {}", new_channel.name);
        }

        Ok(())
    }

    pub(crate) async fn check_left(&self, ctx: &Context, channel_id: ChannelId) -> Result<(), serenity::Error> {
        let channel = channel_id.to_channel(&ctx.http).await?
            .guild().ok_or_else(|| serenity::Error::Other("Channel not in guild"))?;

        let identifier = channel.name.chars().last().unwrap_or_default();
        let channel_type = match VoiceChannelType::by_identifier(identifier) {
            Some(t) => t,
            None => return Ok(()),
        };

        let members = channel.members(&ctx.cache);
        if members.map_or(false, |m| !m.is_empty()) {
            return Ok(());
        }

        let mut cache = self.channel_cache.write().await;
        let current_channels = cache.entry(channel_type).or_insert_with(Vec::new);

        if current_channels.len() > 1 {
            current_channels.retain(|&id| id != channel_id);
            drop(cache);

            if let Err(e) = channel_id.delete(&ctx.http).await {
                eprintln!("Failed to delete empty channel: {}", e);
            } else {
                println!("Deleted empty channel: {}", channel.name);
                self.sort_channels(ctx, channel.guild_id).await?;
            }
        }

        Ok(())
    }

    async fn sort_channels(&self, ctx: &Context, guild_id: serenity::model::id::GuildId) -> Result<(), serenity::Error> {
        let guild_channels = guild_id.channels(&ctx.http).await?;
        let mut voice_channels: Vec<_> = guild_channels.values()
            .filter(|ch| ch.kind == ChannelType::Voice && ch.parent_id == Some(self.category_id))
            .collect();

        voice_channels.sort_by(|a, b| {
            let limit_a = a.user_limit.unwrap_or(0);
            let limit_b = b.user_limit.unwrap_or(0);
            limit_a.cmp(&limit_b)
        });

        let positions: Vec<_> = voice_channels.iter().enumerate()
            .map(|(index, channel)| (channel.id, index as u64))
            .collect();

        if let Err(e) = guild_id.reorder_channels(&ctx.http, positions).await {
            eprintln!("Failed to sort channels: {}", e);
        }

        Ok(())
    }
}
