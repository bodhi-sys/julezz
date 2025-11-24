// src/bot.rs

use teloxide::{prelude::*, utils::command::BotCommands};
use dotenv::dotenv;
use std::env;
use std::sync::Arc;
use julezz::api::JulesClient;

#[derive(BotCommands, Clone)]
#[command(rename_rule = "lowercase", description = "These commands are supported:")]
enum Command {
    #[command(description = "display this text.")]
    Help,
    #[command(description = "list available sessions.")]
    List,
    #[command(description = "send a message to a session.")]
    Send(String),
}

async fn answer(bot: Bot, msg: Message, cmd: Command, client: Arc<JulesClient>) -> ResponseResult<()> {
    match cmd {
        Command::Help => {
            bot.send_message(msg.chat.id, Command::descriptions().to_string()).await?;
        }
        Command::List => {
            match client.list_sessions().await {
                Ok(sessions) => {
                    let mut response = String::from("To send a message, use `/send <session_id> <message>`.\n\nAvailable sessions:\n");
                    for session in sessions {
                        response.push_str(&format!("- `{}`: {}\n", session.id, session.title));
                    }
                    bot.send_message(msg.chat.id, response).await?;
                }
                Err(e) => {
                    log::error!("Failed to list sessions: {:?}", e);
                    bot.send_message(msg.chat.id, "Sorry, something went wrong while listing the sessions.").await?;
                }
            }
        }
        Command::Send(text) => {
            let parts: Vec<&str> = text.splitn(2, ' ').collect();
            if parts.len() == 2 {
                let session_id = parts[0];
                let prompt = parts[1];
                match client.send_message(session_id, prompt).await {
                    Ok(_) => {
                        bot.send_message(msg.chat.id, "Message sent successfully!").await?;
                    }
                    Err(e) => {
                        log::error!("Failed to send message: {:?}", e);
                        bot.send_message(msg.chat.id, "Sorry, something went wrong while sending your message.").await?;
                    }
                }
            } else {
                bot.send_message(msg.chat.id, "Invalid format. Use: /send <session_id> <message>").await?;
            }
        }
    };

    Ok(())
}

pub async fn start_bot() {
    dotenv().ok();
    pretty_env_logger::init();
    log::info!("Starting command bot...");

    let api_key = env::var("JULES_API_KEY").expect("JULES_API_KEY must be set");
    let client = Arc::new(JulesClient::new(Some(api_key)).expect("Failed to create JulesClient"));

    let bot = Bot::from_env();

    let handler = Update::filter_message()
        .branch(dptree::entry().filter_command::<Command>().endpoint(answer));

    Dispatcher::builder(bot, handler)
        .dependencies(dptree::deps![client])
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;
}
