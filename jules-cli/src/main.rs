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
        /// The prompt to send
        prompt: String,
    },
}

#[derive(clap::Subcommand, Debug)]
enum ActivitiesCommands {
    /// List activities for a session
    List {
        /// The ID of the session
        session_id: String,
    },
    /// Get a specific activity from a session
    Get {
        /// The ID of the session
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
                            for session in sessions_list {
                                let state = match session.state.as_str() {
                                    "ACTIVE" => session.state.green(),
                                    "COMPLETED" => session.state.blue(),
                                    _ => session.state.yellow(),
                                };
                                println!("\n- {}: {}", session.id.bold(), session.title);
                                if let Some(source_context) = session.source_context {
                                    if let Some(repo_context) = source_context.github_repo_context {
                                        let repo_name = source_context.source.replace("sources/github/", "");
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
            SessionsCommands::SendMessage { id, prompt } => {
                if let Err(e) = client.send_message(&id, &prompt).await {
                    handle_error(e);
                }
            }
        },
        Commands::Activities { command } => match command {
            ActivitiesCommands::List { session_id } => {
                match client.list_activities(&session_id).await {
                    Ok(activities) => {
                        println!("{}\n", format!("Activities for session {}", session_id).bold().underline());
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
            "list",
        ]);
        assert_eq!(args.api_key, Some("test-key".to_string()));
        assert!(matches!(args.command, Commands::Sources { .. }));
    }
}
