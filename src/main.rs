use std::env;
use chrono::Utc;

#[macro_use]
extern crate lazy_static;
use serenity::async_trait;
use serenity::builder::{CreateEmbed, CreateEmbedAuthor};
use serenity::model::channel::{GuildChannel, Message};
use serenity::model::channel::Channel::Guild;
use serenity::model::event::MessageUpdateEvent;
use serenity::model::gateway::{Activity, Ready};
use serenity::model::id::{ChannelId, GuildId, MessageId};
use serenity::prelude::*;

struct Handler;

lazy_static! {
    static ref GUILD: Mutex<Option<GuildId>> = Mutex::new(None);
    static ref BLACKLIST_CHANNELS: Mutex<Option<Vec<u64>>> = Mutex::new(None);
}

struct EnvVars<'a> {
    discord_token: &'a str,
    log_channel: &'a str,
    blacklist_channels: &'a str,
}

const ENV_VARS: EnvVars = EnvVars {
    discord_token: "discord_token",
    log_channel: "LOG_CHANNEL",
    blacklist_channels: "BLACKLIST_CHANNELS",
};

async fn check_id_blacklist(id: u64) -> bool {
    let blacklist_channels = BLACKLIST_CHANNELS.lock().await;
    let blacklist_channels: Option<&Vec<u64>> = blacklist_channels.as_ref();

    match blacklist_channels {
        Some(blacklist_channels) => {
            if blacklist_channels.contains(&id) {
                return true;
            }
        }
        None => (),
    }

    false
}

async fn check_channel_blacklist(ctx: &Context, channel_id: ChannelId) -> bool {
    if check_id_blacklist(channel_id.0).await {
        return true;
    }

    match channel_id.to_channel(&ctx).await {
        Ok(channel) => {
            let channel = match channel {
                Guild(channel) => channel,
                _ => return false,
            };
            match channel.parent_id {
                Some(parent_id) => {
                    if check_id_blacklist(parent_id.0).await {
                        return true;
                    }
                }
                None => {}
            }
        }
        Err(_) => {}
    }

    return false;
}

#[async_trait]
impl EventHandler for Handler {

    // Store the guild id


    // Set a handler for the `message` event - so that whenever a new message
    // is received - the closure (or function) passed will be called.
    //
    // Event handlers are dispatched through a thread pool, and so multiple
    // events can be dispatched simultaneously.
    /*async fn message(&self, _ctx: Context, msg: Message) {
    }*/

    async fn message_delete(&self, ctx: Context, channel_id: ChannelId, deleted_message_id: MessageId, guild_id: Option<GuildId>) {
        let deleted_message = ctx.cache.message(channel_id, deleted_message_id);
        let deleted_message = match deleted_message {
            Some(message) => message,
            // Can't see who's message was deleted unless the cache was hit so no point of logging it.
            None => {
                println!("Message not found in cache");
                return;
            },
        };

        if deleted_message.author.bot {
            return;
        }


        let guild_guard = GUILD.lock().await;
        let guild: Option<&GuildId> = guild_guard.as_ref();

        let message_guild_id = match guild_id {
            Some(guild_id) => guild_id,
            None => return,
        };

        if message_guild_id.as_ref() != guild.unwrap() {
            return;
        }

        if check_channel_blacklist(&ctx, channel_id).await {
            return;
        }

        let log_channel = env::var(ENV_VARS.log_channel).expect("Expected a channel id variable LOG_CHANNEL");
        let log_channel: u64 = log_channel.parse().unwrap();
        let log_channel = ctx.http.get_channel(log_channel).await;
        let log_channel = match log_channel {
            Ok(channel) => channel,
            Err(why) => {
                println!("Error getting log channel: {:?}", why);
                return;
            }
        };

        let _ = log_channel.id().send_message(&ctx.http, |m| {
            m.embed(|e| {
                let embed: &mut CreateEmbed = e;

                embed.color(0xFF3300);
                embed.footer(|f| {
                    f.text(format!("Message ID: {message_id}", message_id = deleted_message_id));
                    f
                });
                embed.author(|a| {
                    let author: &mut CreateEmbedAuthor = a;
                    author.name(format!("{author}", author = deleted_message.author.name));
                    author.icon_url(deleted_message.author.avatar_url().unwrap_or_else(|| deleted_message.author.default_avatar_url()));
                    a
                });
                // set the embed timestamp as the current system time and not the message timestamp
                embed.timestamp(Utc::now().to_rfc3339());

                embed.description(format!("**Message deleted in** <#{channel_id}>\n**Content**\n{content}",
                                          channel_id = channel_id.0,
                                          content = deleted_message.content));
                embed
            });
            m
        }).await;
    }

    async fn message_update(&self, ctx: Context, old_if_available: Option<Message>, new: Option<Message>, event: MessageUpdateEvent) {


        let msg_author = match event.author {
            Some(author) => author,
            None => return,
        };

        if msg_author.bot {
            return;
        }

        if check_channel_blacklist(&ctx, event.channel_id).await {
            return;
        }


        match event.channel_id.to_channel(&ctx).await {
            Ok(channel) => {
                let channel = match channel {
                    Guild(channel) => channel,
                    _ => return,
                };
                match channel.parent_id {
                    Some(parent_id) => {
                        if check_id_blacklist(parent_id.0).await {
                            return;
                        }
                    }
                    None => {}
                }
            }
            Err(_) => {}
        }

        let guild_guard = GUILD.lock().await;
        let guild: Option<&GuildId> = guild_guard.as_ref();
        let message_guild_id = match event.guild_id {
            Some(guild_id) => guild_id,
            None => return,
        };
        if message_guild_id.0 != guild.unwrap().0 {
            return;
        }

        let old_text = match old_if_available {
            Some(message) => message.content,
            None => "*Message was not cached*".to_string(),
        };

        let new_text = match new {
            Some(message) => message.content,
            None => match event.content {
                Some(content) => content,
                None => return,
            },
        };

        let log_channel = env::var(ENV_VARS.log_channel).expect("Expected a channel id variable LOG_CHANNEL");
        // convert string to u64
        let log_channel: u64 = log_channel.parse().unwrap();
        let log_channel = ctx.http.get_channel(log_channel).await;
        let log_channel = match log_channel {
            Ok(channel) => channel,
            Err(why) => {
                println!("Error getting log channel: {:?}", why);
                return;
            }
        };

        let _ = log_channel.id().send_message(&ctx.http, |m| {
            m.embed(| e | {
                // Not needed, just makes the IDE act slightly nicer
                let embed: &mut CreateEmbed = e;

                embed.color(0x1F6FEB);
                embed.footer(|f| {
                    f.text(format!("Message ID: {message_id}", message_id = event.id));
                    f
                });
                embed.author(|a| {
                    let author: &mut CreateEmbedAuthor = a;
                    author.name(format!("{author}", author = msg_author.name));
                    author.icon_url(msg_author.avatar_url().unwrap_or_else(|| msg_author.default_avatar_url()));
                    a
                });
                match event.timestamp {
                    Some(timestamp) => {
                        embed.timestamp(timestamp);
                    }
                    None => (),
                }
                embed.description(format!("**Message edited in** <#{channel_id}> [View Message]({message_link})\n**Before**\n{old_text}\n\n**After**\n{new_text}",
                                          channel_id = event.channel_id,
                                          message_link = event.id.link(event.channel_id, event.guild_id)));
                embed
            });
            m
        }).await;
    }

    async fn ready(&self, ctx: Context, ready: Ready) {
        println!("{} is connected!", ready.user.name);

        let log_channel = env::var(ENV_VARS.log_channel).expect("Expected a channel id variable LOG_CHANNEL");
        // convert string to u64
        let log_channel: u64 = log_channel.parse().unwrap();
        let log_channel = ctx.http.get_channel(log_channel).await;
        let log_channel = match log_channel {
            Ok(channel) => channel,
            Err(err) => {
                // Throw an error if we can't get the log channel
                panic!("Error getting log channel: {:?}. Please check the ID", err);
            }
        };
        let guild = log_channel.guild();
        let guild: GuildChannel = match guild {
            Some(guild) => guild,
            None => {
                panic!("Error getting guild. Please makee sure the bot is in the server and the ID isn't a DM channel");
            }
        };
        // Set the guild to the global mutex
        let mut guild_mutex = GUILD.lock().await;
        *guild_mutex = Some(guild.guild_id);

        ctx.set_activity(Activity::streaming("Love Tropics '23, Nov 3-5", "https://www.twitch.tv/lovetropics")).await;
    }
}

#[tokio::main]
async fn main() {

    let token = env::var(ENV_VARS.discord_token).expect("Expected a token in the environment variable discord_token");
    let _ = env::var(ENV_VARS.log_channel).expect("Expected a channel id variable LOG_CHANNEL");

    if let Ok(blacklist) = env::var(ENV_VARS.blacklist_channels) {
        let channels: Result<Vec<u64>, _> = blacklist
            .split(',')
            .map(|segment| segment.trim().parse::<u64>().map_err(|e| format!("Failed to parse segment: {}", e)))
            .collect();

        match channels {
            Ok(channels) => {
                let mut blacklist_channels = BLACKLIST_CHANNELS.lock().await;
                *blacklist_channels = Some(channels);
            }
            _ => {}
        }
    }

    println!("Starting bot");

    let intents = GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::DIRECT_MESSAGES
        | GatewayIntents::MESSAGE_CONTENT;

    let mut client = Client::builder(&token, intents)
        .event_handler(Handler)
        .cache_settings(|settings| {
            settings.max_messages(600);
            settings
        })
        .await
        .expect("Err creating client");

    if let Err(why) = client.start().await {
        println!("Client error: {:?}", why);
    }
}
