// src/bot.rs

use teloxide::{prelude::*, types::ParseMode, utils::command::BotCommands};
use dotenv::dotenv;
use std::collections::HashMap;
use std::env;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{self, Duration};
use julezz::api::JulesClient;
use julezz::cache::Cache;
use julezz::resolve::resolve_session_identifier;

fn escape_markdown_v2(text: &str) -> String {
    let mut escaped = String::new();
    for ch in text.chars() {
        match ch {
            '_' | '*' | '[' | ']' | '(' | ')' | '~' | '`' | '>' | '#' | '+' | '-' | '=' | '|' | '{' | '}' | '.' | '!' => {
                escaped.push('\\');
                escaped.push(ch);
            }
            _ => {
                escaped.push(ch);
            }
        }
    }
    escaped
}

#[derive(BotCommands, Clone)]
#[command(rename_rule = "lowercase", description = "These commands are supported:")]
enum Command {
    #[command(description = "display this text.")]
    Help,
    #[command(description = "list available sessions.")]
    List,
    #[command(description = "send a message to a session.")]
    Send(String),
    #[command(description = "authenticate with your Jules API key.")]
    Auth(String),
}

async fn answer(
    bot: Bot,
    msg: Message,
    cmd: Command,
    client: Arc<Mutex<Option<JulesClient>>>,
    cache: Arc<Cache>,
    server_api_key: Arc<String>,
) -> ResponseResult<()> {
    match cmd {
        Command::Help => {
            bot.send_message(msg.chat.id, Command::descriptions().to_string()).await?;
        }
        Command::Auth(api_key) => {
            if api_key == *server_api_key {
                let mut client_guard = client.lock().await;
                *client_guard =
                    Some(JulesClient::new(Some(api_key)).expect("Failed to create JulesClient"));
                bot.send_message(msg.chat.id, "Authentication successful!").await?;

                if cache.read_chat_id().unwrap().is_none() {
                    cache
                        .write_chat_id(&msg.chat.id.to_string())
                        .expect("Failed to write chat ID");
                    bot.send_message(
                        msg.chat.id,
                        "Your chat ID has been saved as the owner.",
                    )
                    .await?;
                }
            } else {
                bot.send_message(msg.chat.id, "Authentication failed: Invalid API key.").await?;
            }
        }
        Command::List => {
            if let Some(client) = &*client.lock().await {
                let aliases = match cache.read_aliases() {
                    Ok(aliases) => aliases,
                    Err(e) => {
                        log::error!("Failed to read aliases: {:?}", e);
                        bot.send_message(msg.chat.id, "Sorry, something went wrong while reading your aliases.").await?;
                        return Ok(());
                    }
                };
                let mut session_aliases: std::collections::HashMap<String, Vec<String>> =
                    std::collections::HashMap::new();
                for (alias, session_id) in aliases {
                    session_aliases.entry(session_id).or_default().push(alias);
                }

                match client.list_sessions().await {
                    Ok(sessions) => {
                        let mut response = String::from("To send a message, use `/send <session_id_or_alias> <message>`.\n\nAvailable sessions:\n");
                    for session in sessions {
                        let alias_str = if let Some(aliases) = session_aliases.get(&session.id) {
                            format!(" ({})", aliases.join(", "))
                        } else {
                            "".to_string()
                        };
                        response.push_str(&format!("- `{}`{}: {}\n", session.id, alias_str, session.title));
                    }
                    bot.send_message(msg.chat.id, response).await?;
                }
                Err(e) => {
                    log::error!("Failed to list sessions: {:?}", e);
                    bot.send_message(msg.chat.id, "Sorry, something went wrong while listing the sessions.").await?;
                }
            }
            } else {
                bot.send_message(msg.chat.id, "You are not authenticated. Please use the `/auth` command to provide your API key.").await?;
            }
        }
        Command::Send(text) => {
            if let Some(client) = &*client.lock().await {
                let parts: Vec<&str> = text.splitn(2, ' ').collect();
                if parts.len() == 2 {
                    let identifier = parts[0];
                    let prompt = parts[1];
                    match client.list_sessions().await {
                        Ok(sessions) => {
                            match resolve_session_identifier(identifier, &sessions) {
                                Ok(session_id) => {
                                    match client.send_message(&session_id, prompt).await {
                                        Ok(_) => {
                                            bot.send_message(msg.chat.id, "Message sent successfully!").await?;
                                        }
                                        Err(e) => {
                                            log::error!("Failed to send message: {:?}", e);
                                            bot.send_message(msg.chat.id, "Sorry, something went wrong while sending your message.").await?;
                                        }
                                    }
                                }
                                Err(e) => {
                                    bot.send_message(msg.chat.id, format!("Error: {}", e)).await?;
                                }
                            }
                        }
                        Err(e) => {
                            log::error!("Failed to list sessions: {:?}", e);
                            bot.send_message(msg.chat.id, "Sorry, something went wrong while listing the sessions.").await?;
                        }
                    }
                } else {
                    bot.send_message(msg.chat.id, "Invalid format. Use: /send <session_id_or_alias> <message>").await?;
                }
            } else {
                bot.send_message(msg.chat.id, "You are not authenticated. Please use the `/auth` command to provide your API key.").await?;
            }
        }
    };

    Ok(())
}

pub async fn start_bot() {
    dotenv().ok();
    pretty_env_logger::init();
    log::info!("Starting command bot...");

    let server_api_key = env::var("JULES_API_KEY").expect("JULES_API_KEY must be set");
    let client: Arc<Mutex<Option<JulesClient>>> = Arc::new(Mutex::new(None));

    let bot = Bot::from_env();

    let last_activities = Arc::new(Mutex::new(HashMap::<String, String>::new()));

    let cache = Arc::new(Cache::new().expect("Failed to create cache"));
    let bot_for_task = bot.clone();
    let client_for_task = client.clone();
    let last_activities_for_task = last_activities.clone();
    let cache_for_task = cache.clone();

    let poll_interval_seconds = env::var("JULEZZ_POLL_INTERVAL_SECONDS")
        .unwrap_or_else(|_| "30".to_string())
        .parse()
        .expect("JULEZZ_POLL_INTERVAL_SECONDS must be an integer");

    tokio::spawn(async move {
        let mut interval = time::interval(Duration::from_secs(poll_interval_seconds));
        loop {
            interval.tick().await;

            if let Some(client) = &*client_for_task.lock().await {
                if let Ok(Some(chat_id_str)) = cache_for_task.read_chat_id() {
                    let chat_id = ChatId(chat_id_str.parse().expect("Chat ID must be an integer"));
                    log::info!("Checking for new activities...");
                    let sessions = match client.list_sessions().await {
                    Ok(sessions) => sessions,
                    Err(e) => {
                        log::error!("Failed to list sessions for activity check: {:?}", e);
                        continue;
                    }
                };

                for session in sessions {
                    let activities = match client.fetch_activities(&session.id).await {
                        Ok(activities) => activities,
                        Err(e) => {
                            log::error!("Failed to fetch activities for session {}: {:?}", session.id, e);
                            continue;
                        }
                    };

                    if let Some(last_activity) = activities.iter().filter(|a| a.originator == "agent").last() {
                        let mut last_activities = last_activities_for_task.lock().await;
                        let last_seen_activity_id = last_activities.get(&session.id).cloned();

                        if last_seen_activity_id.as_deref() != Some(&last_activity.id) {
                            let notification_message = if let Some(agent_messaged) = &last_activity.agent_messaged {
                                Some(format!(
                                    "New message in session *{}*:\n{}",
                                    escape_markdown_v2(&session.title),
                                    escape_markdown_v2(&agent_messaged.agent_message)
                                ))
                            } else if last_activity.plan_generated.is_some() {
                                Some(format!(
                                    "Plan generated for session *{}*.",
                                    escape_markdown_v2(&session.title)
                                ))
                            } else if let Some(progress) = &last_activity.progress_updated {
                                Some(format!(
                                    "Progress update for session *{}*:\n{}",
                                    escape_markdown_v2(&session.title),
                                    escape_markdown_v2(progress.title.as_deref().unwrap_or("No title"))
                                ))
                            } else if last_activity.artifacts.is_some() {
                                Some(format!(
                                    "New artifacts generated for session *{}*.",
                                    escape_markdown_v2(&session.title)
                                ))
                            } else {
                                None
                            };

                            if let Some(message) = notification_message {
                                if let Err(e) = bot_for_task.send_message(chat_id, &message).parse_mode(ParseMode::MarkdownV2).await {
                                    log::error!("Failed to send notification: {:?}", e);
                                }
                            }
                            last_activities.insert(session.id.clone(), last_activity.id.clone());
                        }
                    }
                }
            }
        }
        }
    });

    let handler = Update::filter_message()
        .branch(dptree::entry().filter_command::<Command>().endpoint(answer));

    Dispatcher::builder(bot, handler)
        .dependencies(dptree::deps![client, cache, Arc::new(server_api_key)])
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;
}
