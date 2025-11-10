# Jules CLI

A cool command-line interface for interacting with the Google Jules REST API.

## Installation

1.  Ensure you have Rust and Cargo installed. If not, follow the instructions at [rustup.rs](https://rustup.rs/).
2.  Clone this repository: `git clone <repository-url>`
3.  Navigate to the project directory: `cd jules-cli`
4.  Build the project: `cargo build --release`
5.  The executable will be located at `target/release/jules-cli`.

## Configuration

To use the Jules CLI, you need to provide your Google API key. You can do this in two ways:

1.  **Command-line flag:** Use the `--api-key` flag with any command:

    ```bash
    jules-cli --api-key YOUR_API_KEY sources
    ```

2.  **Environment variable:** Set the `JULES_API_KEY` environment variable:

    ```bash
    export JULES_API_KEY=YOUR_API_KEY
    jules-cli sources
    ```

## Usage

### Sources

List all available sources:

```bash
jules-cli sources
```

### Sessions

**List all sessions:**

```bash
jules-cli sessions list
```

**Create a new session:**

```bash
jules-cli sessions create --source <SOURCE_ID>
```

**Get a specific session:**

```bash
jules-cli sessions get <SESSION_ID>
```

**Approve a plan for a session:**

```bash
jules-cli sessions approve-plan <SESSION_ID>
```

**Send a message to a session:**

```bash
jules-cli sessions send-message <SESSION_ID> "Your message here"
```

### Activities

**List all activities for a session:**

```bash
jules-cli activities list --session-id <SESSION_ID>
```

**Get a specific activity:**

```bash
jules-cli activities get --session-id <SESSION_ID> <ACTIVITY_ID>
```
