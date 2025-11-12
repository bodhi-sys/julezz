use clap::Parser;
use colored::Colorize;
use julezz::api::{handle_error, JulesClient};
use std::fs;

fn get_session_id_from_index(index_str: &str) -> Result<String, String> {
    let index: usize = index_str.parse().map_err(|_| "Invalid index".to_string())?;
    if index == 0 {
        return Err("Index must be greater than 0".to_string());
    }

    if let Some(config_dir) = dirs::config_dir() {
        let sessions_file = config_dir.join("julezz").join("sessions.json");
        if sessions_file.exists() {
            let data = fs::read_to_string(sessions_file).map_err(|_| "Could not read sessions file".to_string())?;
            let session_ids: Vec<String> = serde_json::from_str(&data).map_err(|_| "Could not parse sessions file".to_string())?;
            if let Some(session_id) = session_ids.get(index - 1) {
                return Ok(session_id.clone());
            }
        }
    }
    Err("Session index not found. Run `sessions list` to refresh the cache.".to_string())
}

/// A cool CLI for Google Jules
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Your Google API key
    #[arg(short, long, env = "JULES_API_KEY")]
    api_key: Option<String>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(clap::Subcommand, Debug)]
enum Commands {
    /// Manage sources
    Sources {
        #[command(subcommand)]
        command: SourcesCommands,
    },
    /// Manage sessions
    Sessions {
        #[command(subcommand)]
        command: SessionsCommands,
    },
    /// Manage activities
    Activities {
        #[command(subcommand)]
        command: ActivitiesCommands,
    },
}

#[derive(clap::Subcommand, Debug)]
enum SourcesCommands {
    /// List sources
    List,
    /// Get a source
    Get {
        /// The ID of the source to get
        id: String,
    },
}

#[derive(clap::Subcommand, Debug)]
enum SessionsCommands {
    /// List sessions
    List,
    /// Create a new session
    Create {
        /// The source to use for the session
        #[arg(short, long)]
        source: String,
    },
    /// Get a session by index
    Get {
        /// The index of the session to get
        index: String,
    },
    /// Approve a plan by index
    ApprovePlan {
        /// The index of the session to approve the plan for
        index: String,
    },
    /// Send a message to a session by index
    SendMessage {
        /// The index of the session to send the message to
        index: String,
        /// The prompt to send
        prompt: String,
    },
}

#[derive(clap::Subcommand, Debug)]
enum ActivitiesCommands {
    /// List activities for a session by index
    List {
        /// The index of the session
        index: String,
    },
    /// Get a specific activity from a session by index
    Get {
        /// The index of the session
        index: String,
        /// The ID of the activity to get
        id: String,
    },
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    let client = match JulesClient::new(args.api_key) {
        Ok(client) => client,
        Err(e) => {
            handle_error(e);
            return;
        }
    };

    match args.command {
        Commands::Sources { command } => match command {
            SourcesCommands::List => {
                match client.list_sources().await {
                    Ok(sources) => {
                        println!("Available sources:");
                        for source in sources {
                            println!("- {}: {}", source.id, source.name);
                        }
                    }
                    Err(e) => {
                        handle_error(e);
                    }
                }
            }
            SourcesCommands::Get { id } => {
                match client.get_source(&id).await {
                    Ok(source) => {
                        println!("Source:");
                        println!("- {}: {}", source.id, source.name);
                    }
                    Err(e) => {
                        handle_error(e);
                    }
                }
            }
        },
        Commands::Sessions { command } => match command {
            SessionsCommands::List => {
                match client.list_sessions().await {
                    Ok(sessions_list) => {
                        println!("{}", "Jules Sessions".bold().underline());
                        if sessions_list.is_empty() {
                            println!("No sessions found.");
                        } else {
                            let session_ids: Vec<String> =
                                sessions_list.iter().map(|s| s.id.clone()).collect();
                            if let Some(config_dir) = dirs::config_dir() {
                                let jules_dir = config_dir.join("julezz");
                                if let Err(e) = fs::create_dir_all(&jules_dir) {
                                    eprintln!("{} Could not create config directory: {}", "Error:".red(), e);
                                } else {
                                    let sessions_file = jules_dir.join("sessions.json");
                                    match serde_json::to_string(&session_ids) {
                                        Ok(json) => {
                                            if let Err(e) = fs::write(sessions_file, json) {
                                                eprintln!("{} Could not write to sessions file: {}", "Error:".red(), e);
                                            }
                                        }
                                        Err(e) => {
                                            eprintln!("{} Could not serialize session IDs: {}", "Error:".red(), e);
                                        }
                                    }
                                }
                            }

                            for (i, session) in sessions_list.iter().enumerate() {
                                let state = match session.state.as_str() {
                                    "ACTIVE" => session.state.green(),
                                    "COMPLETED" => session.state.blue(),
                                    _ => session.state.yellow(),
                                };
                                println!(
                                    "\n{}: {}: {}",
                                    (i + 1).to_string().bold(),
                                    session.id.bold(),
                                    session.title
                                );
                                if let Some(source_context) = &session.source_context {
                                    if let Some(repo_context) = &source_context.github_repo_context {
                                        let repo_name =
                                            source_context.source.replace("sources/github/", "");
                                        println!(
                                            "  {} {}/{}",
                                            "Repo:".dimmed(),
                                            repo_name,
                                            repo_context.starting_branch.cyan()
                                        );
                                    }
                                }
                                println!("  {}: {}", "State".dimmed(), state);
                            }
                        }
                    }
                    Err(e) => {
                        handle_error(e);
                    }
                }
            }
            SessionsCommands::Create { source } => {
                match client.create_session(&source).await {
                    Ok(session) => {
                        println!("Session created:");
                        println!("- {}: {} ({})", session.id, session.name, session.state);
                    }
                    Err(e) => {
                        handle_error(e);
                    }
                }
            }
            SessionsCommands::Get { index } => {
                match get_session_id_from_index(&index) {
                    Ok(session_id) => {
                        match client.get_session(&session_id).await {
                            Ok(session) => {
                                println!("Session:");
                                println!("- {}: {} ({})", session.id, session.name, session.state);
                            }
                            Err(e) => {
                                handle_error(e);
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("{} {}", "Error:".red(), e);
                    }
                }
            }
            SessionsCommands::ApprovePlan { index } => {
                match get_session_id_from_index(&index) {
                    Ok(session_id) => {
                        if let Err(e) = client.approve_plan(&session_id).await {
                            handle_error(e);
                        }
                    }
                    Err(e) => {
                        eprintln!("{} {}", "Error:".red(), e);
                    }
                }
            }
            SessionsCommands::SendMessage { index, prompt } => {
                match get_session_id_from_index(&index) {
                    Ok(session_id) => {
                        if let Err(e) = client.send_message(&session_id, &prompt).await {
                            handle_error(e);
                        }
                    }
                    Err(e) => {
                        eprintln!("{} {}", "Error:".red(), e);
                    }
                }
            }
        },
        Commands::Activities { command } => match command {
            ActivitiesCommands::List { index } => {
                match get_session_id_from_index(&index) {
                    Ok(session_id) => {
                        match client.list_activities(&session_id).await {
                            Ok(activities) => {
                                println!(
                                    "{}\n",
                                    format!("Activities for session {}", session_id)
                                        .bold()
                                        .underline()
                                );
                                let mut activities = activities;
                                activities.sort_by(|a, b| a.create_time.cmp(&b.create_time));
                                for activity in activities {
                                    let originator = match activity.originator.as_str() {
                                        "agent" => activity.originator.cyan(),
                                        "user" => activity.originator.green(),
                                        _ => activity.originator.dimmed(),
                                    };
                                    println!(
                                        "[{}] {}",
                                        activity.create_time.dimmed(),
                                        originator
                                    );
                                    if let Some(agent_messaged) = activity.agent_messaged {
                                        if !agent_messaged.agent_message.is_empty() {
                                            println!("  {}", agent_messaged.agent_message);
                                        } else if let Some(progress) = activity.progress_updated {
                                            if let Some(description) = &progress.description {
                                                println!("  {}\n    {}", progress.title.dimmed(), description.dimmed());
                                            } else {
                                                println!("  {}", progress.title.dimmed());
                                            }
                                        } else if let Some(title) = activity.title {
                                            println!("  {}", title.dimmed());
                                        }
                                    } else if let Some(user_messaged) = activity.user_messaged {
                                        println!("  {}", user_messaged.user_message);
                                    } else if activity.plan_approved.is_some() {
                                        println!("  {}", "Plan Approved".yellow());
                                    } else if let Some(progress) = activity.progress_updated {
                                        if let Some(description) = &progress.description {
                                            println!("  {}\n    {}", progress.title.dimmed(), description.dimmed());
                                        } else {
                                            println!("  {}", progress.title.dimmed());
                                        }
                                    } else if let Some(title) = activity.title {
                                        println!("  {}", title.dimmed());
                                    }
                                    println!();
                                }
                            }
                            Err(e) => {
                                handle_error(e);
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("{} {}", "Error:".red(), e);
                    }
                }
            }
            ActivitiesCommands::Get { index, id } => {
                match get_session_id_from_index(&index) {
                    Ok(session_id) => {
                        match client.get_activity(&session_id, &id).await {
                            Ok(activity) => {
                                println!("Activity {}:", id);
                                println!("- {}: {}", activity.id, activity.name);
                            }
                            Err(e) => {
                                handle_error(e);
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("{} {}", "Error:".red(), e);
                    }
                }
            }
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_args() {
        let args = Args::parse_from(&[
            "jules-cli",
            "--api-key",
            "test-key",
            "sources",
            "list",
        ]);
        assert_eq!(args.api_key, Some("test-key".to_string()));
        assert!(matches!(args.command, Commands::Sources { .. }));
    }
}
