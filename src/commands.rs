use std::sync::{Arc, Mutex};

use crate::InputState;

pub mod tiktok;

/// Represents the result of a command execution
#[derive(Debug)]
pub enum CommandResult {
    /// Command was handled successfully with optional output height
    Handled { output_height: usize },
    /// Command was not recognized
    NotRecognized,
}

/// All available commands
#[derive(Debug)]
pub enum Command {
    TikTok,
    // Future commands can be added here
}

impl Command {
    /// Parse a command from input string
    pub fn from_input(input: &str) -> Option<Self> {
        match input.trim() {
            "tiktok" => Some(Command::TikTok),
            _ => None,
        }
    }

    /// Get the command names this enum variant responds to
    pub fn command_names(&self) -> &[&str] {
        match self {
            Command::TikTok => &["tiktok"],
        }
    }

    /// Handle the command execution
    pub async fn handle(
        &self,
        _input: &str,
        state: &mut InputState,
        out: Arc<Mutex<std::io::Stdout>>,
    ) -> anyhow::Result<CommandResult> {
        match self {
            Command::TikTok => {
                tiktok::handle_tiktok_command(state, out).await?;
                Ok(CommandResult::Handled {
                    output_height: tiktok::TIKTOK_ANIMATION_HEIGHT,
                })
            }
        }
    }
}

/// Central command dispatcher that manages all available commands
pub struct CommandDispatcher;

impl CommandDispatcher {
    /// Create a new command dispatcher
    pub fn new() -> Self {
        Self
    }

    /// Process a command by checking all available commands
    pub async fn handle_command(
        &self,
        input: &str,
        state: &mut InputState,
        out: Arc<Mutex<std::io::Stdout>>,
    ) -> anyhow::Result<CommandResult> {
        if let Some(command) = Command::from_input(input) {
            command.handle(input, state, out).await
        } else {
            Ok(CommandResult::NotRecognized)
        }
    }

    /// Get a list of all available commands
    pub fn list_commands(&self) -> Vec<&str> {
        vec!["tiktok"] // Can be generated from Command enum in the future
    }
}

impl Default for CommandDispatcher {
    fn default() -> Self {
        Self::new()
    }
}
