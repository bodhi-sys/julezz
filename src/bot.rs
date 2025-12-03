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
    #[command(description = "approve a plan. Usage: /ok [session_id_or_alias]")]
    Ok(String),
    #[command(description = "create or list aliases. Usage: /alias [@<alias_name> <session_id_or_alias>]")]
    Alias(String),
    #[command(description = "delete an alias. Usage: /unalias @<alias_name>")]
    Unalias(String),
    #[command(description = "delete a session. Usage: /delete <session_id_or_alias>")]
    Delete(String),
    #[command(description = "list activities for a session. Usage: /activities <session_id_or_alias>")]
    Activities(String),
    #[command(description = "list available sources.")]
    Src,
    #[command(description = "create a new session. Usage: /new --source <source> --branch <branch> <title>")]
    New(String),
    #[command(description = "get a session by identifier. Usage: /get <session_id_or_alias>")]
    Get(String),
    #[command(description = "merge the pull request for a session. Usage: /merge <session_id_or_alias>")]
    Merge(String),
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
                        let cached_sessions: Vec<CachedSession> = sessions_list
                            .iter()
                            .map(|session| CachedSession {
                                id: session.id.clone(),
                                title: session.title.clone(),
                                source_context: session.source_context.clone(),
                                pull_request_url: session.pull_request_url.clone(),
                            })
                            .collect();

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
        Command::Get(identifier) => {
            if let Some(client) = &*client.lock().await {
                match client.list_sessions().await {
                    Ok(sessions) => {
                        match resolve_session_identifier(&identifier, &sessions) {
                            Ok(session_id) => {
                                match client.get_session(&session_id).await {
                                    Ok(session) => {
                                        let response = format!("Session details:\n- ID: {}\n- Title: {}\n- State: {}", session.id, session.title, session.state.unwrap_or_default());
                                        bot.send_message(msg.chat.id, response).await?;
                                    }
                                    Err(e) => {
                                        log::error!("Failed to get session: {:?}", e);
                                        bot.send_message(msg.chat.id, "Sorry, something went wrong while getting the session.").await?;
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
                bot.send_message(msg.chat.id, "You are not authenticated. Please use the `/auth` command to provide your API key.").await?;
            }
        }
        Command::New(text) => {
            if let Some(client) = &*client.lock().await {
                let mut source = None;
                let mut branch = "main".to_string();
                let mut title = None;

                let parts: Vec<&str> = text.split_whitespace().collect();
                let mut i = 0;
                while i < parts.len() {
                    match parts[i] {
                        "--source" => {
                            if i + 1 < parts.len() {
                                source = Some(parts[i + 1].to_string());
                                i += 2;
                            } else {
                                bot.send_message(msg.chat.id, "Missing value for --source").await?;
                                return Ok(());
                            }
                        }
                        "--branch" => {
                            if i + 1 < parts.len() {
                                branch = parts[i + 1].to_string();
                                i += 2;
                            } else {
                                bot.send_message(msg.chat.id, "Missing value for --branch").await?;
                                return Ok(());
                            }
                        }
                        _ => {
                            title = Some(parts[i..].join(" "));
                            break;
                        }
                    }
                }

                if let (Some(source), Some(title)) = (source, title) {
                    match client.create_session(&source, &title, true, &branch).await {
                        Ok(session) => {
                            bot.send_message(msg.chat.id, format!("Session created: {} ({})", session.id, session.title)).await?;
                        }
                        Err(e) => {
                            log::error!("Failed to create session: {:?}", e);
                            bot.send_message(msg.chat.id, "Sorry, something went wrong while creating the session.").await?;
                        }
                    }
                } else {
                    bot.send_message(msg.chat.id, "Invalid format. Usage: /create --source <source> --branch <branch> <title>").await?;
                }
            } else {
                bot.send_message(msg.chat.id, "You are not authenticated. Please use the `/auth` command to provide your API key.").await?;
            }
        }
        Command::Src => {
            if let Some(client) = &*client.lock().await {
                match client.list_sources().await {
                    Ok(sources) => {
                        let mut response = String::from("Available sources:\n");
                        for source in sources {
                            response.push_str(&format!("- {}\n", source.name));
                        }
                        bot.send_message(msg.chat.id, response).await?;
                    }
                    Err(e) => {
                        log::error!("Failed to list sources: {:?}", e);
                        bot.send_message(msg.chat.id, "Sorry, something went wrong while listing the sources.").await?;
                    }
                }
            } else {
                bot.send_message(msg.chat.id, "You are not authenticated. Please use the `/auth` command to provide your API key.").await?;
            }
        }
        Command::Activities(identifier) => {
            if let Some(client) = &*client.lock().await {
                match client.list_sessions().await {
                    Ok(sessions) => {
                        match resolve_session_identifier(&identifier, &sessions) {
                            Ok(session_id) => {
                                match client.fetch_activities(&session_id).await {
                                    Ok(activities) => {
                                        let response = format_activities(&activities, 5, &session_id);
                                        bot.send_message(msg.chat.id, response).await?;
                                    }
                                    Err(e) => {
                                        log::error!("Failed to fetch activities: {:?}", e);
                                        bot.send_message(msg.chat.id, "Sorry, something went wrong while fetching activities.").await?;
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
                bot.send_message(msg.chat.id, "You are not authenticated. Please use the `/auth` command to provide your API key.").await?;
            }
        }
        Command::Delete(identifier) => {
            if let Some(client) = &*client.lock().await {
                match client.list_sessions().await {
                    Ok(sessions) => {
                        match resolve_session_identifier(&identifier, &sessions) {
                            Ok(session_id) => {
                                match client.delete_session(&session_id).await {
                                    Ok(_) => {
                                        // Update cache and aliases
                                        let mut cached_sessions = cache.read_sessions().unwrap_or_default();
                                        cached_sessions.retain(|s| s.id != session_id);
                                        cache.write_sessions(&cached_sessions).unwrap_or_default();

                                        let mut aliases = cache.read_aliases().unwrap_or_default();
                                        aliases.retain(|_, s_id| *s_id != session_id);
                                        cache.write_aliases(&aliases).unwrap_or_default();

                                        bot.send_message(msg.chat.id, format!("Session {} deleted.", session_id)).await?;
                                    }
                                    Err(e) => {
                                        log::error!("Failed to delete session: {:?}", e);
                                        bot.send_message(msg.chat.id, "Sorry, something went wrong while deleting the session.").await?;
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
                bot.send_message(msg.chat.id, "You are not authenticated. Please use the `/auth` command to provide your API key.").await?;
            }
        }
        Command::Alias(text) => {
            if client.lock().await.is_some() {
                if text.is_empty() {
                    // List aliases
                    match cache.read_aliases() {
                        Ok(aliases) => {
                            if aliases.is_empty() {
                                bot.send_message(msg.chat.id, "No aliases found.").await?;
                            } else {
                                let mut response = String::from("Aliases:\n");
                                for (alias, session_id) in aliases {
                                    response.push_str(&format!("- {} -> {}\n", alias, session_id));
                                }
                                bot.send_message(msg.chat.id, response).await?;
                            }
                        }
                        Err(e) => {
                            log::error!("Failed to read aliases: {:?}", e);
                            bot.send_message(msg.chat.id, "Sorry, something went wrong while reading your aliases.").await?;
                        }
                    }
                } else {
                    let parts: Vec<&str> = text.splitn(2, ' ').collect();
                    if parts.len() == 2 {
                        let alias_name = parts[0];
                        let identifier = parts[1];

                        if !alias_name.starts_with('@') {
                            bot.send_message(msg.chat.id, "Alias must start with '@'").await?;
                            return Ok(());
                        }

                        let client_guard = client.lock().await;
                        let client = client_guard.as_ref().unwrap();

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
                }
            } else {
                bot.send_message(msg.chat.id, "You are not authenticated. Please use the `/auth` command to provide your API key.").await?;
            }
        }
        Command::Unalias(alias_name) => {
            if client.lock().await.is_some() {
                if !alias_name.starts_with('@') {
                    bot.send_message(msg.chat.id, "Alias must start with '@'").await?;
                    return Ok(());
                }

                let mut aliases = match cache.read_aliases() {
                    Ok(aliases) => aliases,
                    Err(e) => {
                        log::error!("Failed to read aliases: {:?}", e);
                        bot.send_message(msg.chat.id, "Sorry, something went wrong while reading your aliases.").await?;
                        return Ok(());
                    }
                };

                if aliases.remove(&alias_name).is_some() {
                    if let Err(e) = cache.write_aliases(&aliases) {
                        log::error!("Failed to write aliases: {:?}", e);
                        bot.send_message(msg.chat.id, "Sorry, something went wrong while deleting your alias.").await?;
                    } else {
                        bot.send_message(msg.chat.id, format!("Alias '{}' deleted.", alias_name)).await?;
                    }
                } else {
                    bot.send_message(msg.chat.id, format!("Alias '{}' not found.", alias_name)).await?;
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

        Command::Ok(identifier) => {
            if let Some(client) = &*client.lock().await {
                let session_id_result = if identifier.is_empty() {
                    cache.read_current_session()
                } else {
                    match client.list_sessions().await {
                        Ok(sessions) => resolve_session_identifier(&identifier, &sessions).map(Some),
                        Err(e) => Err(e.to_string()),
                    }
                };

                match session_id_result {
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

                         bot.send_message(msg.chat.id, "No current session is set. Use /s <session_id_or_alias> to set one, or provide an identifier.").await?;
                    }
                    Err(e) => {
                        bot.send_message(msg.chat.id, format!("Error: {}", e)).await?;
                    }
                }
            } else {
                bot.send_message(msg.chat.id, "You are not authenticated. Please use the `/auth` command to provide your API key.").await?;
            }
        }
        Command::Merge(identifier) => {
            if let Some(client) = &*client.lock().await {
                match client.list_sessions().await {
                    Ok(sessions) => {
                        match resolve_session_identifier(&identifier, &sessions) {
                            Ok(session_id) => {
                                let session = sessions.iter().find(|s| s.id == session_id);
                                if let Some(session) = session {
                                    if let Some(pull_request_url) = &session.pull_request_url {
                                        if let Err(e) = client.merge_pull_request(pull_request_url) {
                                            log::error!("Failed to merge pull request: {:?}", e);
                                            bot.send_message(msg.chat.id, "Sorry, something went wrong while merging the pull request.").await?;
                                        } else {
                                            bot.send_message(msg.chat.id, "Pull request merged successfully!").await?;
                                        }
                                    } else {
                                        bot.send_message(msg.chat.id, "No pull request URL found for this session.").await?;
                                    }
                                } else {
                                    bot.send_message(msg.chat.id, "Session not found.").await?;
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
    };

    Ok(())
}


fn format_activities(activities: &[julezz::api::Activity], n: usize, session_id: &str) -> String {
    let mut response = format!("Activities for session {}:\n\n", session_id);
    let mut activities = activities.to_vec();
    activities.sort_by(|a, b| a.create_time.cmp(&b.create_time));
    let activities_to_show = activities.iter().rev().take(n).rev();

    for activity in activities_to_show {
        response.push_str(&format!("[{}] {}\n", activity.create_time, activity.originator));

        if let Some(agent_messaged) = &activity.agent_messaged {
            if !agent_messaged.agent_message.is_empty() {
                response.push_str(&format!("  {}\n", agent_messaged.agent_message));
            }
        } else if let Some(user_messaged) = &activity.user_messaged {
            response.push_str(&format!("  {}\n", user_messaged.user_message));
        } else if let Some(plan_generated) = &activity.plan_generated {
            response.push_str("  Plan Generated\n");
            for step in &plan_generated.plan.steps {
                response.push_str(&format!("    - {}\n", step.title));
            }
        } else if activity.plan_approved.is_some() {
            response.push_str("  Plan Approved\n");
        } else if activity.session_completed.is_some() {
            response.push_str("  Session Completed\n");
        } else if let Some(progress) = &activity.progress_updated {
            if let Some(title) = &progress.title {
                response.push_str(&format!("  {}\n", title));
            }
            if let Some(description) = &progress.description {
                response.push_str(&format!("    {}\n", description));
            }
        } else if let Some(artifacts) = &activity.artifacts {
            for artifact in artifacts {
                if let Some(bash_output) = &artifact.bash_output {
                    response.push_str(&format!("  $ {}\n", bash_output.command));
                    response.push_str(&format!("    {}\n", bash_output.output));
                }
                if let Some(change_set) = &artifact.change_set {
                    response.push_str("  Code Change\n");
                    if let Some(patch) = &change_set.git_patch.unidiff_patch {
                        response.push_str(&format!("{}\n", patch));
                    }
                }
            }
        } else if let Some(title) = &activity.title {
            response.push_str(&format!("  {}\n", title));
        }

        response.push('\n');
    }
    response
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
