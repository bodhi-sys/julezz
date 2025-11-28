// Copyright 2024 Google LLC
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     https://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! A command-line interface for the Jules API.

use clap::{CommandFactory, Parser};
use colored::Colorize;
use julezz::api::{handle_error, JulesClient, Session};
use std::io;

mod bot;
use julezz::cache::{Cache, CachedSession};
use julezz::resolve::{resolve_session_identifier, resolve_session_identifier_and_index};

fn get_sessions_from_cache() -> Result<Vec<Session>, String> {
    let cache = Cache::new()?;
    let sessions = cache.read_sessions()?;
    let api_sessions: Vec<Session> = sessions
        .into_iter()
        .map(|s| Session {
            name: s.title.clone(),
            id: s.id,
            state: None,
            title: s.title,
            source_context: s.source_context,
        })
        .collect();
    Ok(api_sessions)
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
    /// Start the Telegram bot
    Bot {
        #[command(subcommand)]
        command: BotCommands,
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
    /// Manage session aliases
    Alias {
        /// The alias to create or delete
        alias: Option<String>,
        /// The session number to associate with the alias
        session_number: Option<usize>,
        /// Delete the specified alias
        #[arg(short, long)]
        delete: bool,
    },
    /// List sessions
    List,
    /// Create a new session
    Create {
        /// The source to use for the session
        #[arg(short, long)]
        source: String,
        /// The title of the session
        #[arg(last = true)]
        title: String,
        /// The branch to use for the session
        #[arg(short, long, default_value = "main")]
        branch: String,
        /// Disable automatically creating a pull request
        #[arg(long)]
        no_auto_pr: bool,
        /// Alias to create for the new session
        #[arg(short, long)]
        alias: Option<String>,
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
    /// Delete a session by index
    Delete {
        /// The index of the session to delete
        index: String,
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
        /// Show raw JSON output
        #[arg(long)]
        raw: bool,
    },
    /// Get a specific activity from a session by index
    Get {
        /// The index of the session
        index: String,
        /// The ID of the activity to get
        id: String,
    },
}

#[derive(clap::Subcommand, Debug)]
enum BotCommands {
    /// Start the bot
    Start,
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
                            println!("{}", source.name);
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
            SessionsCommands::Alias { alias, session_number, delete } => {
                if let Err(e) = manage_aliases(alias, session_number, delete) {
                    eprintln!("{} {}", "Error:".red(), e);
                }
            }
            SessionsCommands::List => {
                match client.list_sessions().await {
                    Ok(sessions_list) => {
                        println!("{}", "Jules Sessions".bold().underline());
                        if sessions_list.is_empty() {
                            println!("No sessions found.");
                        } else {
                            if let Err(e) = manage_sessions_cache(&sessions_list) {
                                eprintln!("{} {}", "Error:".red(), e);
                            }
                        }
                    }
                    Err(e) => {
                        handle_error(e);
                    }
                }
            }
            SessionsCommands::Create { source, title, branch, no_auto_pr, alias } => {
                match client.create_session(&source, &title, !no_auto_pr, &branch).await {
                    Ok(session) => {
                        println!("Session created:");
                        println!("- {}: {} ({})", session.id, session.name, session.state.clone().unwrap_or_default());

                        if let Some(alias_name) = alias {
                            if let Err(e) = add_alias_for_new_session(&session, &alias_name) {
                                eprintln!("{} {}", "Error creating alias:".red(), e);
                            }
                        }
                    }
                    Err(e) => {
                        handle_error(e);
                    }
                }
            }
            SessionsCommands::Get { index } => {
                match get_sessions_from_cache() {
                    Ok(sessions) => {
                        match resolve_session_identifier(&index, &sessions) {
                            Ok(session_id) => {
                                match client.get_session(&session_id).await {
                                    Ok(session) => {
                                        println!("Session:");
                                        println!("- {}: {} ({})", session.id, session.name, session.state.unwrap_or_default());
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
                    Err(e) => {
                        eprintln!("{} {}", "Error:".red(), e);
                    }
                }
            }
            SessionsCommands::ApprovePlan { index } => {
                match get_sessions_from_cache() {
                    Ok(sessions) => {
                        match resolve_session_identifier(&index, &sessions) {
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
                    Err(e) => {
                        eprintln!("{} {}", "Error:".red(), e);
                    }
                }
            }
            SessionsCommands::Delete { index } => {
                match get_sessions_from_cache() {
                    Ok(sessions) => {
                        match resolve_session_identifier_and_index(&index, &sessions) {
                            Ok((session_id, session_index)) => {
                                match client.delete_session(&session_id).await {
                                    Ok(_) => {
                                        println!("Session {} deleted.", session_id);
                                        if let Err(e) = remove_session_from_cache(session_index).and_then(|_| update_aliases_after_deletion(&session_id)) {
                                            eprintln!("{} {}", "Error updating local state:".red(), e);
                                            eprintln!("{}", "Your local state may be out of sync with the server.".yellow());
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
                    Err(e) => {
                        eprintln!("{} {}", "Error:".red(), e);
                    }
                }
            }
            SessionsCommands::SendMessage { index, prompt } => {
                match get_sessions_from_cache() {
                    Ok(sessions) => {
                        match resolve_session_identifier(&index, &sessions) {
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
                    Err(e) => {
                        eprintln!("{} {}", "Error:".red(), e);
                    }
                }
            }
        },
        Commands::Activities { command } => match command {
            ActivitiesCommands::Fetch { index } => {
                match get_sessions_from_cache() {
                    Ok(sessions) => {
                        match resolve_session_identifier_and_index(&index, &sessions) {
                            Ok((session_id, session_index)) => {
                                let cache = Cache::new().unwrap();
                                let session = cache.read_sessions().unwrap().remove(session_index - 1);
                                match client.fetch_activities(&session_id).await {
                                    Ok(activities) => {
                                        print_activities(&activities, activities.len(), &session);
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
                    Err(e) => {
                        eprintln!("{} {}", "Error:".red(), e);
                    }
                }
            }
            ActivitiesCommands::List { index, n, r, raw } => {
                match get_sessions_from_cache() {
                    Ok(sessions) => {
                        match resolve_session_identifier_and_index(&index, &sessions) {
                            Ok((session_id, session_index)) => {
                                let cache = Cache::new().unwrap();
                                let session = cache.read_sessions().unwrap().remove(session_index - 1);
                                let activities_result = if r {
                                    client.fetch_activities(&session_id).await
                                } else {
                                    client.list_cached_activities(&session_id)
                                };

                                match activities_result {
                                    Ok(activities) => {
                                        if raw {
                                            if let Ok(json) = serde_json::to_string_pretty(&activities) {
                                                println!("{}", json);
                                            } else {
                                                eprintln!("{} {}", "Error:".red(), "Could not serialize activities to JSON");
                                            }
                                        } else {
                                            print_activities(&activities, n, &session);
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
                    Err(e) => {
                        eprintln!("{} {}", "Error:".red(), e);
                    }
                }
            }
            ActivitiesCommands::Get { index, id } => {
                match get_sessions_from_cache() {
                    Ok(sessions) => {
                        match resolve_session_identifier(&index, &sessions) {
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
                    Err(e) => {
                        eprintln!("{} {}", "Error:".red(), e);
                    }
                }
            }
        },
        Commands::Bot { command } => match command {
            BotCommands::Start => {
                bot::start_bot().await;
            }
        },
        Commands::Completions { shell } => {
            let mut cmd = Args::command();
            let name = cmd.get_name().to_string();
            clap_complete::generate(shell, &mut cmd, name, &mut io::stdout());
        }
        Commands::__CarapaceSpec { .. } => {
            if let Err(e) = generate_carapace_spec() {
                eprintln!("{} {}", "Error:".red(), e);
            }
        }
        Commands::ListCachedSessionsForCompletion => {
            if let Err(e) = list_cached_sessions_for_completion() {
                eprintln!("{} {}", "Error:".red(), e);
            }
        }
    }
}

/// Synchronizes the local session cache with the list of sessions from the API.
///
/// This function ensures that the local cache is up-to-date with the server.
/// It removes any sessions from the cache that are no longer on the server,
/// and adds any new sessions from the server to the cache.
fn manage_sessions_cache(sessions_list: &[julezz::api::Session]) -> Result<(), String> {
    let cache = Cache::new()?;
    let mut cached_sessions = cache.read_sessions()?;

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

    cache.write_sessions(&cached_sessions)?;

    let session_states: std::collections::HashMap<_, _> = sessions_list
        .iter()
        .map(|s| (s.id.as_str(), s.state.as_deref().unwrap_or("UNKNOWN")))
        .collect();

    let mut aliases = cache.read_aliases()?;
    aliases.retain(|_, session_id| live_session_ids.contains(session_id.as_str()));
    cache.write_aliases(&aliases)?;

    let mut session_aliases: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();
    for (alias, session_id) in aliases {
        session_aliases.entry(session_id).or_default().push(alias);
    }

    for (i, session) in cached_sessions.iter().enumerate() {
        let state_str = session_states.get(session.id.as_str()).unwrap_or(&"UNKNOWN");
        let state = match *state_str {
            "ACTIVE" => state_str.green(),
            "COMPLETED" => state_str.blue(),
            _ => state_str.red(),
        };

        let alias_str = if let Some(aliases) = session_aliases.get(&session.id) {
            format!(" ({}) ", aliases.join(", ")).yellow()
        } else {
            "".yellow()
        };

        println!(
            "\n{}:{}{}: {}",
            (i + 1).to_string().bold(),
            alias_str,
            session.id.bold(),
            session.title
        );
        println!("  {}: {}", "State".dimmed(), state);
    }

    Ok(())
}

/// Removes a session from the local cache by its 1-based index.
fn remove_session_from_cache(index: usize) -> Result<(), String> {
    let cache = Cache::new()?;
    let mut sessions = cache.read_sessions()?;
    if index > 0 && index <= sessions.len() {
        sessions.remove(index - 1);
        cache.write_sessions(&sessions)?;
    }
    Ok(())
}

/// Removes any aliases that point to a deleted session.
fn update_aliases_after_deletion(deleted_session_id: &str) -> Result<(), String> {
    let cache = Cache::new()?;
    let mut aliases = cache.read_aliases()?;
    aliases.retain(|_, session_id| session_id != deleted_session_id);
    cache.write_aliases(&aliases)
}

/// Generates a Carapace spec for shell completions.
fn generate_carapace_spec() -> Result<(), String> {
    let mut cmd = Args::command();
    let mut buffer = Vec::new();
    clap_complete::generate(carapace_spec_clap::Spec, &mut cmd, "julezz", &mut buffer);
    let mut yaml_spec: serde_yaml::Value =
        serde_yaml::from_slice(&buffer).map_err(|e| format!("Could not parse YAML spec: {}", e))?;

    if let Some(mapping) = yaml_spec
        .get_mut("completion")
        .and_then(|c| c.get_mut("commands"))
        .and_then(|c| c.as_sequence_mut())
    {
        for item in mapping {
            if let Some(item_map) = item.as_mapping_mut() {
                let name = item_map.get("name").and_then(|n| n.as_str());
                if name == Some("sessions") {
                    if let Some(commands) =
                        item_map.get_mut("commands").and_then(|c| c.as_sequence_mut())
                    {
                        for session_cmd in commands {
                            if let Some(args) = session_cmd
                                .get_mut("arguments")
                                .and_then(|a| a.as_sequence_mut())
                            {
                                for arg in args {
                                    if let Some(arg_map) = arg.as_mapping_mut() {
                                        if arg_map.get("name").and_then(|n| n.as_str())
                                            == Some("index")
                                        {
                                            arg_map.insert(
                                                serde_yaml::Value::String("completion".to_string()),
                                                serde_yaml::Value::Sequence(vec![
                                                    serde_yaml::Value::String(
                                                        "julezz list-cached-sessions-for-completion"
                                                            .to_string(),
                                                    ),
                                                ]),
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
    println!(
        "{}",
        serde_yaml::to_string(&yaml_spec)
            .map_err(|e| format!("Could not serialize YAML spec: {}", e))?
    );
    Ok(())
}

/// Lists the cached sessions for shell completion purposes.
fn list_cached_sessions_for_completion() -> Result<(), String> {
    let cache = Cache::new()?;
    let sessions = cache.read_sessions()?;
    for (i, session) in sessions.iter().enumerate() {
        println!("{}\t{}", i + 1, session.title);
    }
    Ok(())
}

/// Creates an alias for a newly created session.
fn add_alias_for_new_session(
    session: &julezz::api::Session,
    alias_name: &str,
) -> Result<(), String> {
    let cache = Cache::new()?;
    let mut sessions = cache.read_sessions()?;
    sessions.push(CachedSession {
        id: session.id.clone(),
        title: session.title.clone(),
        source_context: session.source_context.clone(),
    });
    cache.write_sessions(&sessions)?;

    let mut aliases = cache.read_aliases()?;
    aliases.insert(alias_name.to_string(), session.id.clone());
    cache.write_aliases(&aliases)
}

/// Manages session aliases.
///
/// This function handles the creation, deletion, and listing of aliases.
fn manage_aliases(
    alias: Option<String>,
    session_number: Option<usize>,
    delete: bool,
) -> Result<(), String> {
    let cache = Cache::new()?;
    let mut aliases = cache.read_aliases()?;

    if delete {
        if let Some(alias_name) = alias {
            if alias_name.starts_with('@') {
                if aliases.remove(&alias_name).is_some() {
                    cache.write_aliases(&aliases)?;
                    println!("Alias '{}' deleted.", alias_name);
                } else {
                    return Err(format!("Alias '{}' not found.", alias_name));
                }
            } else {
                return Err("Alias must start with '@'".to_string());
            }
        } else {
            return Err("Alias to delete must be specified".to_string());
        }
    } else if let (Some(alias_name), Some(number)) = (alias, session_number) {
        if alias_name.starts_with('@') {
            let sessions = get_sessions_from_cache()?;
            let (session_id, _) =
                resolve_session_identifier_and_index(&number.to_string(), &sessions)?;
            aliases.insert(alias_name.clone(), session_id.clone());
            cache.write_aliases(&aliases)?;
            println!(
                "Alias '{}' created for session {} ({}).",
                alias_name, number, session_id
            );
        } else {
            return Err("Alias must start with '@'".to_string());
        }
    } else if aliases.is_empty() {
        println!("No aliases found.");
    } else {
        println!("Aliases:");
        for (alias, session_id) in aliases {
            println!("  {} -> {}", alias, session_id);
        }
    }

    Ok(())
}





#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_args() {
        let args = Args::parse_from(&[
            "julezz",
            "--api-key",
            "test-key",
            "sources",
            "list",
        ]);
        assert_eq!(args.api_key, Some("test-key".to_string()));
        assert!(matches!(args.command, Commands::Sources { .. }));
    }
}

fn print_activities(activities: &[julezz::api::Activity], n: usize, session: &CachedSession) {
    println!(
        "{}\n",
        format!("Activities for session {}", session.id)
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
        println!("[{}] {}", activity.create_time.dimmed(), originator);

        if let Some(agent_messaged) = &activity.agent_messaged {
            if !agent_messaged.agent_message.is_empty() {
                println!("  {}", agent_messaged.agent_message);
            }
        } else if let Some(user_messaged) = &activity.user_messaged {
            println!("  {}", user_messaged.user_message);
        } else if let Some(plan_generated) = &activity.plan_generated {
            println!("  {}", "Plan Generated".yellow());
            for step in &plan_generated.plan.steps {
                println!("    - {}", step.title);
            }
        } else if activity.plan_approved.is_some() {
            println!("  {}", "Plan Approved".yellow());
        } else if activity.session_completed.is_some() {
            println!("  {}", "Session Completed".blue());
        } else if let Some(progress) = &activity.progress_updated {
            if let Some(title) = &progress.title {
                println!("  {}", title.dimmed());
            }
            if let Some(description) = &progress.description {
                println!("    {}", description.dimmed());
            }
        } else if let Some(artifacts) = &activity.artifacts {
            for artifact in artifacts {
                if let Some(bash_output) = &artifact.bash_output {
                    println!("  {}", format!("$ {}", bash_output.command).blue());
                    println!("    {}", bash_output.output);
                }
                if let Some(change_set) = &artifact.change_set {
                    let branch = session
                        .source_context
                        .as_ref()
                        .and_then(|sc| sc.github_repo_context.as_ref())
                        .map(|ghc| ghc.starting_branch.as_str())
                        .unwrap_or("unknown branch");
                    println!("  {} on {}", "Code Change".blue(), branch.yellow());
                    if let Some(patch) = &change_set.git_patch.unidiff_patch {
                        println!("{}", patch);
                    }
                }
            }
        } else if let Some(title) = &activity.title {
            println!("  {}", title.dimmed());
        }

        println!();
    }
}
