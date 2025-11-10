use clap::Parser;
use colored::Colorize;
use jules_cli::api::{handle_error, JulesClient};

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
    /// List available sources
    Sources,
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
enum SessionsCommands {
    /// List sessions
    List,
    /// Create a new session
    Create {
        /// The source to use for the session
        #[arg(short, long)]
        source: String,
    },
    /// Get a session
    Get {
        /// The ID of the session to get
        id: String,
    },
    /// Approve a plan
    ApprovePlan {
        /// The ID of the session to approve the plan for
        id: String,
    },
    /// Send a message to a session
    SendMessage {
        /// The ID of the session to send the message to
        id: String,
        /// The message to send
        message: String,
    },
}

#[derive(clap::Subcommand, Debug)]
enum ActivitiesCommands {
    /// List activities
    List {
        /// The ID of the session to list activities for
        #[arg(short, long)]
        session_id: String,
    },
    /// Get an activity
    Get {
        /// The ID of the session
        #[arg(short, long)]
        session_id: String,
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
        Commands::Sources => {
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
        Commands::Sessions { command } => match command {
            SessionsCommands::List => {
                match client.list_sessions().await {
                    Ok(sessions) => {
                        println!("{}", "Jules Sessions".bold().underline());
                        if sessions.is_empty() {
                            println!("No sessions found.");
                        } else {
                            for session in sessions {
                                let state = match session.state.as_str() {
                                    "ACTIVE" => session.state.green(),
                                    "COMPLETED" => session.state.blue(),
                                    _ => session.state.yellow(),
                                };
                                println!("\n- {}: {}", session.id.bold(), session.name);
                                if let Some(source_context) = session.source_context {
                                    if let Some(git_source) = source_context.git_source {
                                        println!(
                                            "  {} {}/{}",
                                            "Repo:".dimmed(),
                                            git_source.repo,
                                            git_source.branch.cyan()
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
            SessionsCommands::Get { id } => {
                match client.get_session(&id).await {
                    Ok(session) => {
                        println!("Session:");
                        println!("- {}: {} ({})", session.id, session.name, session.state);
                    }
                    Err(e) => {
                        handle_error(e);
                    }
                }
            }
            SessionsCommands::ApprovePlan { id } => {
                if let Err(e) = client.approve_plan(&id).await {
                    handle_error(e);
                }
            }
            SessionsCommands::SendMessage { id, message } => {
                if let Err(e) = client.send_message(&id, &message).await {
                    handle_error(e);
                }
            }
        },
        Commands::Activities { command } => match command {
            ActivitiesCommands::List { session_id } => {
                match client.list_activities(&session_id).await {
                    Ok(activities) => {
                        println!("Activities for session {}:", session_id);
                        for activity in activities {
                            println!("- {}: {}", activity.id, activity.name);
                        }
                    }
                    Err(e) => {
                        handle_error(e);
                    }
                }
            }
            ActivitiesCommands::Get { session_id, id } => {
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
        ]);
        assert_eq!(args.api_key, Some("test-key".to_string()));
        assert!(matches!(args.command, Commands::Sources));
    }
}
