use clap::{CommandFactory, Parser};
use colored::Colorize;
use julezz::api::{handle_error, JulesClient};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use clap_complete::Generator;

#[derive(Serialize, Deserialize)]
struct CachedSession {
    id: String,
    title: String,
}

fn get_session_id_from_index(index_str: &str) -> Result<String, String> {
    let index: usize = index_str.parse().map_err(|_| "Invalid index".to_string())?;
    if index == 0 {
        return Err("Index must be greater than 0".to_string());
    }

    if let Some(config_dir) = dirs::config_dir() {
        let sessions_file = config_dir.join("julezz").join("sessions.json");
        if sessions_file.exists() {
            let data = fs::read_to_string(sessions_file).map_err(|_| "Could not read sessions file".to_string())?;
            let sessions: Vec<CachedSession> = serde_json::from_str(&data).map_err(|_| "Could not parse sessions file".to_string())?;
            if let Some(session) = sessions.get(index - 1) {
                return Ok(session.id.clone());
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
    /// Generate shell completions
    Completions {
        /// The shell to generate completions for
        #[arg(value_enum)]
        shell: clap_complete::Shell,
    },
    #[command(hide = true, name = "__carapace_spec")]
    __CarapaceSpec {
        /// The spec to generate
        spec: String,
    },
    #[command(hide = true)]
    ListCachedSessionsForCompletion,
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
        /// Disable automatically creating a pull request
        #[arg(long)]
        no_auto_pr: bool,
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
    /// Fetch activities for a session by index
    Fetch {
        /// The index of the session
        index: String,
    },
    /// List cached activities for a session by index
    List {
        /// The index of the session
        index: String,
        /// Number of last messages to show
        #[arg(short, long, default_value_t = 5)]
        n: usize,
        /// Re-fetch messages before listing
        #[arg(short, long)]
        r: bool,
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
                        for source in sources {
                            println!("{}", source.id);
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
                            let cached_sessions: Vec<CachedSession> = sessions_list
                                .iter()
                                .map(|s| CachedSession {
                                    id: s.id.clone(),
                                    title: s.title.clone(),
                                })
                                .collect();
                            if let Some(config_dir) = dirs::config_dir() {
                                let jules_dir = config_dir.join("julezz");
                                if let Err(e) = fs::create_dir_all(&jules_dir) {
                                    eprintln!("{} Could not create config directory: {}", "Error:".red(), e);
                                } else {
                                    let sessions_file = jules_dir.join("sessions.json");
                                    match serde_json::to_string(&cached_sessions) {
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
            SessionsCommands::Create { source, no_auto_pr } => {
                match client.create_session(&source, !no_auto_pr).await {
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
            ActivitiesCommands::Fetch { index } => {
                match get_session_id_from_index(&index) {
                    Ok(session_id) => {
                        match client.fetch_activities(&session_id).await {
                            Ok(activities) => {
                                print_activities(&activities, activities.len(), &session_id);
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
            ActivitiesCommands::List { index, n, r } => {
                match get_session_id_from_index(&index) {
                    Ok(session_id) => {
                        let activities_result = if r {
                            client.fetch_activities(&session_id).await
                        } else {
                            client.list_cached_activities(&session_id)
                        };

                        match activities_result {
                            Ok(activities) => {
                                print_activities(&activities, n, &session_id);
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
        Commands::Completions { shell } => {
            let mut cmd = Args::command();
            let name = cmd.get_name().to_string();
            clap_complete::generate(shell, &mut cmd, name, &mut io::stdout());
        }
        Commands::__CarapaceSpec { .. } => {
            let mut cmd = Args::command();
            let mut buffer = Vec::new();
            carapace_spec_clap::Spec.generate(&mut cmd, &mut buffer);
            let mut yaml_spec: serde_yaml::Value = serde_yaml::from_slice(&buffer).unwrap();

            if let Some(mapping) = yaml_spec.get_mut("completion").and_then(|c| c.get_mut("commands")).and_then(|c| c.as_sequence_mut()) {
                for item in mapping {
                    if let Some(item_map) = item.as_mapping_mut() {
                        let name = item_map.get("name").and_then(|n| n.as_str());
                        if name == Some("sessions") {
                            if let Some(commands) = item_map.get_mut("commands").and_then(|c| c.as_sequence_mut()) {
                                for session_cmd in commands {
                                    if let Some(args) = session_cmd.get_mut("arguments").and_then(|a| a.as_sequence_mut()) {
                                        for arg in args {
                                            if let Some(arg_map) = arg.as_mapping_mut() {
                                                if arg_map.get("name").and_then(|n| n.as_str()) == Some("index") {
                                                    arg_map.insert(
                                                        serde_yaml::Value::String("completion".to_string()),
                                                        serde_yaml::Value::Sequence(vec![serde_yaml::Value::String("julezz list-cached-sessions-for-completion".to_string())])
                                                    );
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            println!("{}", serde_yaml::to_string(&yaml_spec).unwrap());
        }
        Commands::ListCachedSessionsForCompletion => {
            if let Some(config_dir) = dirs::config_dir() {
                let sessions_file = config_dir.join("julezz").join("sessions.json");
                if sessions_file.exists() {
                    let data = fs::read_to_string(sessions_file).unwrap();
                    let sessions: Vec<CachedSession> = serde_json::from_str(&data).unwrap();
                    for (i, session) in sessions.iter().enumerate() {
                        println!("{}\t{}", i + 1, session.title);
                    }
                }
            }
        }
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

fn print_activities(activities: &[julezz::api::Activity], n: usize, session_id: &str) {
    println!(
        "{}\n",
        format!("Activities for session {}", session_id)
            .bold()
            .underline()
    );
    let mut activities = activities.to_vec();
    activities.sort_by(|a, b| a.create_time.cmp(&b.create_time));
    let activities_to_show = activities.iter().rev().take(n).rev();

    for activity in activities_to_show {
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
        if let Some(agent_messaged) = &activity.agent_messaged {
            if !agent_messaged.agent_message.is_empty() {
                println!("  {}", agent_messaged.agent_message);
            } else if let Some(progress) = &activity.progress_updated {
                if let Some(description) = &progress.description {
                    println!("  {}\n    {}", progress.title.clone().unwrap_or_default().dimmed(), description.dimmed());
                } else {
                    println!("  {}", progress.title.clone().unwrap_or_default().dimmed());
                }
            } else if let Some(title) = &activity.title {
                println!("  {}", title.dimmed());
            }
        } else if let Some(user_messaged) = &activity.user_messaged {
            println!("  {}", user_messaged.user_message);
        } else if activity.plan_approved.is_some() {
            println!("  {}", "Plan Approved".yellow());
        } else if let Some(progress) = &activity.progress_updated {
            if let Some(description) = &progress.description {
                println!("  {}\n    {}", progress.title.clone().unwrap_or_default().dimmed(), description.dimmed());
            } else {
                println!("  {}", progress.title.clone().unwrap_or_default().dimmed());
            }
        } else if let Some(title) = &activity.title {
            println!("  {}", title.dimmed());
        }
        println!();
    }
}
