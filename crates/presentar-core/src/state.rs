//! State management for Presentar applications.
//!
//! This module implements the Elm Architecture pattern for predictable state
//! management: `State + Message → (State, Command)`.
//!
//! # Examples
//!
//! ```
//! use presentar_core::{State, Command};
//! use serde::{Deserialize, Serialize};
//!
//! // Define your application state
//! #[derive(Clone, Default, Serialize, Deserialize)]
//! struct AppState {
//!     count: i32,
//! }
//!
//! // Define messages that modify state
//! enum AppMessage {
//!     Increment,
//!     Decrement,
//!     Reset,
//! }
//!
//! impl State for AppState {
//!     type Message = AppMessage;
//!
//!     fn update(&mut self, msg: Self::Message) -> Command<Self::Message> {
//!         match msg {
//!             AppMessage::Increment => self.count += 1,
//!             AppMessage::Decrement => self.count -= 1,
//!             AppMessage::Reset => self.count = 0,
//!         }
//!         Command::None
//!     }
//! }
//!
//! let mut state = AppState::default();
//! state.update(AppMessage::Increment);
//! assert_eq!(state.count, 1);
//! ```

use serde::{Deserialize, Serialize};
use std::future::Future;
use std::pin::Pin;

/// Application state trait.
///
/// Implements the Elm Architecture: State + Message → (State, Command)
pub trait State: Clone + Serialize + for<'de> Deserialize<'de> + Send + Sync {
    /// Message type for state updates
    type Message: Send;

    /// Update state in response to a message.
    ///
    /// Returns a command for side effects (async operations, navigation, etc.)
    fn update(&mut self, msg: Self::Message) -> Command<Self::Message>;
}

/// Commands for side effects.
///
/// Commands represent effects that should happen after a state update:
/// - Async tasks (data fetching, file operations)
/// - Navigation
/// - State persistence
#[derive(Default)]
pub enum Command<M> {
    /// No command
    #[default]
    None,
    /// Execute multiple commands
    Batch(Vec<Command<M>>),
    /// Execute an async task
    Task(Pin<Box<dyn Future<Output = M> + Send>>),
    /// Navigate to a route
    Navigate {
        /// Route path
        route: String,
    },
    /// Save state to storage
    SaveState {
        /// Storage key
        key: String,
    },
    /// Load state from storage
    LoadState {
        /// Storage key
        key: String,
        /// Callback with loaded state
        on_load: fn(Option<Vec<u8>>) -> M,
    },
}

impl<M> Command<M> {
    /// Create a task command from an async block.
    pub fn task<F>(future: F) -> Self
    where
        F: Future<Output = M> + Send + 'static,
    {
        Self::Task(Box::pin(future))
    }

    /// Create a batch of commands.
    pub fn batch(commands: impl IntoIterator<Item = Self>) -> Self {
        Self::Batch(commands.into_iter().collect())
    }

    /// Check if this is the none command.
    #[must_use]
    pub const fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }

    /// Map the message type using a function.
    pub fn map<N, F>(self, f: F) -> Command<N>
    where
        F: Fn(M) -> N + Send + Sync + 'static,
        M: Send + 'static,
        N: Send + 'static,
    {
        match self {
            Self::None => Command::None,
            Self::Batch(cmds) => {
                let f = std::sync::Arc::new(f);
                Command::Batch(
                    cmds.into_iter()
                        .map(|c| {
                            let f = f.clone();
                            c.map(move |m| f(m))
                        })
                        .collect(),
                )
            }
            Self::Task(fut) => Command::Task(Box::pin(async move { f(fut.await) })),
            Self::Navigate { route } => Command::Navigate { route },
            Self::SaveState { key } => Command::SaveState { key },
            Self::LoadState { .. } => {
                // Can't easily map LoadState due to function pointer
                // In practice, LoadState is usually at the top level
                Command::None
            }
        }
    }
}

/// A simple counter state for testing.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CounterState {
    /// Current count
    pub count: i32,
}

/// Messages for the counter state.
#[derive(Debug, Clone)]
pub enum CounterMessage {
    /// Increment the counter
    Increment,
    /// Decrement the counter
    Decrement,
    /// Set the counter to a specific value
    Set(i32),
    /// Reset to zero
    Reset,
}

impl State for CounterState {
    type Message = CounterMessage;

    fn update(&mut self, msg: Self::Message) -> Command<Self::Message> {
        match msg {
            CounterMessage::Increment => self.count += 1,
            CounterMessage::Decrement => self.count -= 1,
            CounterMessage::Set(value) => self.count = value,
            CounterMessage::Reset => self.count = 0,
        }
        Command::None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_counter_increment() {
        let mut state = CounterState::default();
        state.update(CounterMessage::Increment);
        assert_eq!(state.count, 1);
    }

    #[test]
    fn test_counter_decrement() {
        let mut state = CounterState { count: 5 };
        state.update(CounterMessage::Decrement);
        assert_eq!(state.count, 4);
    }

    #[test]
    fn test_counter_set() {
        let mut state = CounterState::default();
        state.update(CounterMessage::Set(42));
        assert_eq!(state.count, 42);
    }

    #[test]
    fn test_counter_reset() {
        let mut state = CounterState { count: 100 };
        state.update(CounterMessage::Reset);
        assert_eq!(state.count, 0);
    }

    #[test]
    fn test_command_none() {
        let cmd: Command<()> = Command::None;
        assert!(cmd.is_none());
    }

    #[test]
    fn test_command_default() {
        let cmd: Command<()> = Command::default();
        assert!(cmd.is_none());
    }

    #[test]
    fn test_command_batch() {
        let cmd: Command<i32> = Command::batch([
            Command::Navigate {
                route: "/a".to_string(),
            },
            Command::Navigate {
                route: "/b".to_string(),
            },
        ]);
        assert!(!cmd.is_none());
        if let Command::Batch(cmds) = cmd {
            assert_eq!(cmds.len(), 2);
        } else {
            panic!("Expected Batch command");
        }
    }

    #[test]
    fn test_command_navigate() {
        let cmd: Command<()> = Command::Navigate {
            route: "/home".to_string(),
        };
        if let Command::Navigate { route } = cmd {
            assert_eq!(route, "/home");
        } else {
            panic!("Expected Navigate command");
        }
    }

    #[test]
    fn test_command_save_state() {
        let cmd: Command<()> = Command::SaveState {
            key: "app_state".to_string(),
        };
        if let Command::SaveState { key } = cmd {
            assert_eq!(key, "app_state");
        } else {
            panic!("Expected SaveState command");
        }
    }

    #[test]
    fn test_counter_serialization() {
        let state = CounterState { count: 42 };
        let json = serde_json::to_string(&state).unwrap();
        let loaded: CounterState = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.count, 42);
    }
}
