// src/bot.rs

use teloxide::{prelude::*, types::ParseMode, utils::command::BotCommands};
use dotenv::dotenv;
use std::collections::HashMap;
use std::env;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{self, Duration};
use julezz::api::JulesClient;
use julezz::cache::{Cache, CachedSession};
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
    #[command(description = "switch current session.")]
    S(String),
    #[command(description = "approve plan for the current session.")]
    Ok,
    #[command(description = "create an alias for a session. Usage: /alias @<alias_name> <session_id_or_alias>")]
    Alias(String),
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
            let help_text = format!(
                "{}\n\nOnce you have set a current session with the `/s` command, you can send messages to it directly without using any command.",
                Command::descriptions()
            );
            bot.send_message(msg.chat.id, help_text).await?;
        }
        Command::Auth(api_key) => {
            if api_key == *server_api_key {
                let jules_client = match JulesClient::new(Some(api_key)) {
                    Ok(client) => client,
                    Err(e) => {
                        log::error!("Failed to create JulesClient: {:?}", e);
                        bot.send_message(msg.chat.id, "Authentication failed: Could not create API client.").await?;
                        return Ok(());
                    }
                };

                let mut client_guard = client.lock().await;
                *client_guard = Some(jules_client);

                match cache.read_chat_id() {
                    Ok(Some(_)) => {
                        bot.send_message(msg.chat.id, "Authentication successful! You are already the owner.").await?;
                    }
                    Ok(None) => {
                        if let Err(e) = cache.write_chat_id(&msg.chat.id.to_string()) {
                            log::error!("Failed to write chat ID: {:?}", e);
                            bot.send_message(msg.chat.id, "Authentication successful, but failed to save your chat ID as the owner.").await?;
                        } else {
                            bot.send_message(msg.chat.id, "Authentication successful! Your chat ID has been saved as the owner.").await?;
                        }
                    }
                    Err(e) => {
                        log::error!("Failed to read chat ID: {:?}", e);
                        bot.send_message(msg.chat.id, "Authentication successful, but failed to check your owner status due to a server error.").await?;
                    }
                }
            } else {
                bot.send_message(msg.chat.id, "Authentication failed: Invalid API key.").await?;
            }
        }
        Command::List => {
            if let Some(client) = &*client.lock().await {
                match client.list_sessions().await {
                    Ok(sessions_list) => {
                        let mut cached_sessions = match cache.read_sessions() {
                            Ok(sessions) => sessions,
                            Err(e) => {
                                log::error!("Failed to read cached sessions: {:?}", e);
                                vec![]
                            }
                        };

                        let live_session_ids: std::collections::HashSet<_> =
                            sessions_list.iter().map(|s| s.id.as_str()).collect();
                        cached_sessions.retain(|cs| live_session_ids.contains(cs.id.as_str()));

                        let cached_session_ids: std::collections::HashSet<_> =
                            cached_sessions.iter().map(|cs| cs.id.clone()).collect();
                        for session in sessions_list.iter() {
                            if !cached_session_ids.contains(&session.id) {
                                cached_sessions.push(CachedSession {
                                    id: session.id.clone(),
                                    title: session.title.clone(),
                                    source_context: session.source_context.clone(),
                                });
                            }
                        }

                        if let Err(e) = cache.write_sessions(&cached_sessions) {
                            log::error!("Failed to write sessions to cache: {:?}", e);
                        }

                        let aliases = match cache.read_aliases() {
                            Ok(aliases) => aliases,
                            Err(e) => {
                                log::error!("Failed to read aliases: {:?}", e);
                                std::collections::HashMap::new()
                            }
                        };
                        let mut session_aliases: std::collections::HashMap<String, Vec<String>> =
                            std::collections::HashMap::new();
                        for (alias, session_id) in aliases {
                            session_aliases.entry(session_id).or_default().push(alias);
                        }

                        let mut response = String::from("Available sessions:\n");
                        for (i, session) in cached_sessions.iter().enumerate() {
                            let alias_str = if let Some(aliases) = session_aliases.get(&session.id) {
                                format!(" ({})", aliases.join(", "))
                            } else {
                                "".to_string()
                            };
                            response.push_str(&format!("{}: {}{}: {}\n", i + 1, session.id, alias_str, session.title));
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
        Command::Alias(text) => {
            if let Some(client) = &*client.lock().await {
                let parts: Vec<&str> = text.splitn(2, ' ').collect();
                if parts.len() == 2 {
                    let alias_name = parts[0];
                    let identifier = parts[1];

                    if !alias_name.starts_with('@') {
                        bot.send_message(msg.chat.id, "Alias must start with '@'").await?;
                        return Ok(());
                    }

                    match client.list_sessions().await {
                        Ok(sessions) => {
                            match resolve_session_identifier(identifier, &sessions) {
                                Ok(session_id) => {
                                    let mut aliases = match cache.read_aliases() {
                                        Ok(aliases) => aliases,
                                        Err(e) => {
                                            log::error!("Failed to read aliases: {:?}", e);
                                            bot.send_message(msg.chat.id, "Sorry, something went wrong while reading your aliases.").await?;
                                            return Ok(());
                                        }
                                    };
                                    aliases.insert(alias_name.to_string(), session_id.clone());
                                    if let Err(e) = cache.write_aliases(&aliases) {
                                        log::error!("Failed to write aliases: {:?}", e);
                                        bot.send_message(msg.chat.id, "Sorry, something went wrong while saving your alias.").await?;
                                    } else {
                                        bot.send_message(msg.chat.id, format!("Alias '{}' created for session {}", alias_name, session_id)).await?;
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
                    bot.send_message(msg.chat.id, "Invalid format. Use: /alias @<alias_name> <session_id_or_alias>").await?;
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
        Command::S(identifier) => {
            if let Some(client) = &*client.lock().await {
                match client.list_sessions().await {
                    Ok(sessions) => {
                        match resolve_session_identifier(&identifier, &sessions) {
                            Ok(session_id) => {
                                if let Err(e) = cache.write_current_session(&session_id) {
                                    log::error!("Failed to write current session: {:?}", e);
                                    bot.send_message(msg.chat.id, "Sorry, something went wrong while setting the current session.").await?;
                                } else {
                                    bot.send_message(msg.chat.id, format!("Current session set to {}", session_id)).await?;
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
                bot.send_message(msg.chat.id, "You are not authenticated. Please use the `/auth` command to provide your API key.").await?;
            }
        }
        Command::Ok => {
            if let Some(client) = &*client.lock().await {
                match cache.read_current_session() {
                    Ok(Some(session_id)) => {
                        match client.approve_plan(&session_id).await {
                            Ok(_) => {
                                bot.send_message(msg.chat.id, "Plan approved successfully!").await?;
                            }
                            Err(e) => {
                                log::error!("Failed to approve plan: {:?}", e);
                                bot.send_message(msg.chat.id, "Sorry, something went wrong while approving the plan.").await?;
                            }
                        }
                    }
                    Ok(None) => {
                        bot.send_message(msg.chat.id, "No current session is set. Use /s <session_id_or_alias> to set one.").await?;
                    }
                    Err(e) => {
                        log::error!("Failed to read current session: {:?}", e);
                        bot.send_message(msg.chat.id, "Sorry, something went wrong while reading the current session.").await?;
                    }
                }
            } else {
                bot.send_message(msg.chat.id, "You are not authenticated. Please use the `/auth` command to provide your API key.").await?;
            }
        }
    };

    Ok(())
}

async fn default_message_handler(
    bot: Bot,
    msg: Message,
    client: Arc<Mutex<Option<JulesClient>>>,
    cache: Arc<Cache>,
) -> ResponseResult<()> {
    if let Some(text) = msg.text() {
        if let Some(client) = &*client.lock().await {
            match cache.read_current_session() {
                Ok(Some(session_id)) => {
                    match client.send_message(&session_id, text).await {
                        Ok(_) => {
                            // Do not send a confirmation message to keep the chat clean
                        }
                        Err(e) => {
                            log::error!("Failed to send message: {:?}", e);
                            bot.send_message(msg.chat.id, "Sorry, something went wrong while sending your message.").await?;
                        }
                    }
                }
                Ok(None) => {
                    bot.send_message(msg.chat.id, "No current session is set. Use /s <session_id_or_alias> to set one.").await?;
                }
                Err(e) => {
                    log::error!("Failed to read current session: {:?}", e);
                    bot.send_message(msg.chat.id, "Sorry, something went wrong while reading the current session.").await?;
                }
            }
        } else {
            bot.send_message(msg.chat.id, "You are not authenticated. Please use the `/auth` command to provide your API key.").await?;
        }
    }
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
                match cache_for_task.read_chat_id() {
                    Ok(Some(chat_id_str)) => {
                        let chat_id = match chat_id_str.parse() {
                            Ok(id) => ChatId(id),
                            Err(e) => {
                                log::error!("Failed to parse chat ID '{}': {:?}", chat_id_str, e);
                                continue;
                            }
                        };
                        log::info!("Checking for new activities...");
                        let sessions = match client.list_sessions().await {
                            Ok(sessions) => sessions,
                    Err(e) => {
                        log::error!("Failed to list sessions for activity check: {:?}", e);
                        continue;
                    }
                };

                let aliases = match cache_for_task.read_aliases() {
                    Ok(aliases) => aliases,
                    Err(e) => {
                        log::error!("Failed to read aliases for notification task: {:?}", e);
                        // Continue with an empty alias map
                        HashMap::new()
                    }
                };

                let mut session_aliases: HashMap<String, Vec<String>> = HashMap::new();
                for (alias, session_id) in aliases {
                    session_aliases.entry(session_id).or_default().push(alias);
                }

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
                            let session_display = if let Some(aliases) = session_aliases.get(&session.id) {
                                let formatted_aliases = aliases.iter().map(|a| escape_markdown_v2(a)).collect::<Vec<_>>().join(", ");
                                format!("[{}]", formatted_aliases)
                            } else {
                                format!("*{}*", escape_markdown_v2(&session.title))
                            };

                            let notification_message = if let Some(agent_messaged) = &last_activity.agent_messaged {
                                Some(format!(
                                    "New message in session {}:\n{}",
                                    session_display,
                                    escape_markdown_v2(&agent_messaged.agent_message)
                                ))
                            } else if last_activity.plan_generated.is_some() {
                                Some(format!(
                                    "Plan generated for session {}\\.",
                                    session_display
                                ))
                            } else if let Some(progress) = &last_activity.progress_updated {
                                Some(format!(
                                    "Progress update for session {}:\n{}",
                                    session_display,
                                    escape_markdown_v2(progress.title.as_deref().unwrap_or("No title"))
                                ))
                            } else if last_activity.artifacts.is_some() {
                                Some(format!(
                                    "New artifacts generated for session {}\\.",
                                    session_display
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
                    Err(e) => {
                        log::error!("Failed to read chat ID: {:?}", e);
                    }
                    Ok(None) => {
                        // No owner chat ID set yet, do nothing.
                    }
                }
            }
        }
    });

    let handler = Update::filter_message()
        .branch(dptree::entry().filter_command::<Command>().endpoint(answer))
        .branch(dptree::endpoint(default_message_handler));

    Dispatcher::builder(bot, handler)
        .dependencies(dptree::deps![client, cache, Arc::new(server_api_key)])
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;
}
