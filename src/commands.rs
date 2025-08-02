use std::sync::{Arc, Mutex};
use async_trait::async_trait;

use crate::InputState;

pub mod tiktok;

/// Represents the result of a command execution
#[derive(Debug)]
pub enum CommandResult {
    /// Command was handled successfully
    Handled,
    /// Command was not recognized
    NotRecognized,
}

/// Trait for command handlers
#[async_trait]
pub trait CommandHandler: Send + Sync {
    /// Handle a command and return whether it was processed
    async fn handle(
        &self,
        input: &str,
        state: &mut InputState,
        out: Arc<Mutex<std::io::Stdout>>,
    ) -> anyhow::Result<CommandResult>;

    /// Get the command name(s) this handler responds to
    fn command_names(&self) -> &[&str];
}

/// Central command dispatcher that manages all available commands
pub struct CommandDispatcher {
    handlers: Vec<Box<dyn CommandHandler>>,
}

impl CommandDispatcher {
    /// Create a new command dispatcher with all available command handlers
    pub fn new() -> Self {
        let mut dispatcher = Self {
            handlers: Vec::new(),
        };
        
        // Register all command handlers
        dispatcher.register_handler(Box::new(tiktok::TikTokCommand));
        
        dispatcher
    }
    
    /// Register a new command handler
    pub fn register_handler(&mut self, handler: Box<dyn CommandHandler>) {
        self.handlers.push(handler);
    }
    
    /// Process a command by checking all registered handlers
    pub async fn handle_command(
        &self,
        input: &str,
        state: &mut InputState,
        out: Arc<Mutex<std::io::Stdout>>,
    ) -> anyhow::Result<CommandResult> {
        for handler in &self.handlers {
            match handler.handle(input, state, out.clone()).await? {
                CommandResult::Handled => return Ok(CommandResult::Handled),
                CommandResult::NotRecognized => continue,
            }
        }
        
        Ok(CommandResult::NotRecognized)
    }
    
    /// Get a list of all available commands
    pub fn list_commands(&self) -> Vec<&str> {
        self.handlers
            .iter()
            .flat_map(|handler| handler.command_names().iter().copied())
            .collect()
    }
}

impl Default for CommandDispatcher {
    fn default() -> Self {
        Self::new()
    }
}