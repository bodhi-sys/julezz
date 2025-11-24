# Julezz

Julezz is a command-line interface (CLI) for the Google Jules API, designed to streamline your workflow and provide a powerful, terminal-based experience for managing your Jules sessions.

## Features

- **Session Management**: List, create, and delete Jules sessions directly from your terminal.
- **Alias System**: Create convenient shortcuts for your session IDs, making it easier to work with multiple sessions.
- **Activity Tracking**: Fetch and view the activity history for any session.
- **Source Management**: List and inspect available sources for creating new sessions.
- **Shell Completions**: Generate completion scripts for your favorite shell to speed up your workflow.

## Installation

To build Julezz from source, you will need to have the Rust toolchain installed. You can install it from [rustup.rs](https://rustup.rs/).

Once you have Rust installed, you can clone this repository and build the project:

```bash
git clone https://github.com/your-username/julezz.git
cd julezz
cargo build --release
```

The compiled binary will be located at `target/release/julezz`. You can copy this binary to a location in your `PATH` (e.g., `/usr/local/bin`) to make it accessible from anywhere in your terminal.

## Authentication

Julezz requires a Google API key to authenticate with the Jules API. You can provide this key in one of two ways:

1.  **Environment Variable**: Set the `JULES_API_KEY` environment variable to your API key.

    ```bash
    export JULES_API_KEY="your-api-key"
    ```

2.  **Command-Line Flag**: Use the `--api-key` flag when running any command.

    ```bash
    julezz --api-key "your-api-key" sessions list
    ```

## Usage

Here is a brief overview of the available commands. For more detailed information, you can use the `--help` flag with any command (e.g., `julezz sessions --help`).

### Sessions

-   **List Sessions**: `julezz sessions list`
    -   Displays a list of all your Jules sessions, along with their indices, IDs, and any associated aliases.
-   **Create a Session**: `julezz sessions create --source <source> --branch <branch> "<title>"`
    -   Creates a new session with the specified source, branch, and title.
-   **Delete a Session**: `julezz sessions delete <index|alias>`
    -   Deletes a session by its index or alias.
-   **Manage Aliases**:
    -   `julezz sessions alias`: Lists all aliases.
    -   `julezz sessions alias @my-alias <index>`: Creates an alias for a session.
    -   `julezz sessions alias --delete @my-alias`: Deletes an alias.

### Activities

-   **Fetch Activities**: `julezz activities fetch <index|alias>`
    -   Fetches and caches the full activity history for a session.
-   **List Activities**: `julezz activities list <index|alias>`
    -   Displays the most recent activities for a session from the local cache.

### Sources

-   **List Sources**: `julezz sources list`
    -   Lists all available sources that you can use to create new sessions.

### Telegram Bot

Julezz includes a Telegram bot for interacting with your sessions in a conversational way. The bot can also send you notifications when there are new messages from the agent.

**Setup**

1.  **Create a Telegram Bot**: Talk to the [BotFather](https://t.me/botfather) on Telegram to create a new bot. You will receive a token; keep it safe.
2.  **Set Environment Variables**: The bot requires the `JULES_API_KEY` and `TELOXIDE_TOKEN` environment variables to be set. You can find your chat ID in the next step.
3.  **Find Your Chat ID**: Start the bot and send it the `/whoami` command. The bot will reply with your chat ID. Add this ID to your environment variables as `TELEGRAM_CHAT_ID`.
    *   `JULES_API_KEY`: Your Google API key for the Jules API. This is the key the bot will use to authenticate with the Jules API.
    *   `TELOXIDE_TOKEN`: The token you received from the BotFather.
    *   `TELEGRAM_CHAT_ID`: Your chat ID, which the bot will use to send you notifications.
    *   `JULEZZ_POLL_INTERVAL_SECONDS` (optional): The interval in seconds at which the bot checks for new messages. Defaults to 30.

    You can set these in your shell or create a `.env` file in the project's root directory:
    ```
    JULES_API_KEY=your-api-key
    TELOXIDE_TOKEN=your-telegram-bot-token
    TELEGRAM_CHAT_ID=your-chat-id
    JULEZZ_POLL_INTERVAL_SECONDS=30
    ```

**Running the Bot**

Once the environment variables are set, you can start the bot with the following command:

```bash
julezz bot start
```

The bot will start listening for commands.

**Commands**

-   `/auth <api_key>`: Authenticates the bot with your Jules API key. This must be done before any other commands can be used.
-   `/help`: Shows a list of all available commands.
-   `/whoami`: Replies with your Telegram chat ID, which you can use for the `TELEGRAM_CHAT_ID` environment variable.
-   `/list`: Displays all your active Jules sessions. The output will show the session ID and title for each session.
-   `/send <identifier> <message>`: Sends a message to a specific session.
    -   `<identifier>`: The session's ID, alias (e.g., `@my-session`), or numeric index from the `julezz sessions list` command.
    -   `<message>`: The text you want to send.

    *Example*:
    ```
    /send @my-session Hello, can you help me with a new feature?
    ```

## Alias System

The alias system allows you to assign a memorable name to a session ID. This is particularly useful when you are working with multiple sessions, as it saves you from having to remember or look up session IDs.

Aliases are linked to the permanent session ID, not the temporary index shown in the `sessions list` command. This means that even if you delete a session, your aliases for other sessions will remain valid.

## Shell Completions

Julezz can generate completion scripts for various shells, including Bash, Zsh, Fish, and PowerShell. To generate a script, use the `completions` command:

```bash
julezz completions <your-shell>
```

Follow the instructions provided by the command to install the completion script for your shell.
