//! Command runtime for executing side effects.
//!
//! This module provides the infrastructure to execute `Command` values
//! produced by state updates.

use crate::state::Command;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Router trait for navigation commands.
pub trait Router: Send + Sync {
    /// Navigate to a route.
    fn navigate(&self, route: &str);

    /// Get the current route.
    fn current_route(&self) -> String;
}

/// Storage trait for state persistence commands.
pub trait Storage: Send + Sync {
    /// Save data to storage.
    fn save(&self, key: &str, data: &[u8]);

    /// Load data from storage.
    fn load(&self, key: &str) -> Option<Vec<u8>>;

    /// Remove data from storage.
    fn remove(&self, key: &str);

    /// Check if a key exists.
    fn contains(&self, key: &str) -> bool;
}

/// In-memory storage for testing.
#[derive(Debug, Default)]
pub struct MemoryStorage {
    data: Mutex<HashMap<String, Vec<u8>>>,
}

impl MemoryStorage {
    /// Create a new empty memory storage.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Get the number of stored items.
    #[must_use]
    pub fn len(&self) -> usize {
        self.data.lock().expect("MemoryStorage mutex poisoned").len()
    }

    /// Check if storage is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.data.lock().expect("MemoryStorage mutex poisoned").is_empty()
    }

    /// Clear all stored data.
    pub fn clear(&self) {
        self.data.lock().expect("MemoryStorage mutex poisoned").clear();
    }
}

impl Storage for MemoryStorage {
    fn save(&self, key: &str, data: &[u8]) {
        self.data
            .lock()
            .expect("MemoryStorage mutex poisoned")
            .insert(key.to_string(), data.to_vec());
    }

    fn load(&self, key: &str) -> Option<Vec<u8>> {
        self.data.lock().expect("MemoryStorage mutex poisoned").get(key).cloned()
    }

    fn remove(&self, key: &str) {
        self.data.lock().expect("MemoryStorage mutex poisoned").remove(key);
    }

    fn contains(&self, key: &str) -> bool {
        self.data.lock().expect("MemoryStorage mutex poisoned").contains_key(key)
    }
}

/// In-memory router for testing.
#[derive(Debug)]
pub struct MemoryRouter {
    route: Mutex<String>,
    history: Mutex<Vec<String>>,
}

impl Default for MemoryRouter {
    fn default() -> Self {
        Self::new()
    }
}

impl MemoryRouter {
    /// Create a new memory router.
    #[must_use]
    pub fn new() -> Self {
        Self {
            route: Mutex::new("/".to_string()),
            history: Mutex::new(vec!["/".to_string()]),
        }
    }

    /// Get navigation history.
    #[must_use]
    pub fn history(&self) -> Vec<String> {
        self.history.lock().expect("MemoryRouter mutex poisoned").clone()
    }

    /// Get history length.
    #[must_use]
    pub fn history_len(&self) -> usize {
        self.history.lock().expect("MemoryRouter mutex poisoned").len()
    }
}

impl Router for MemoryRouter {
    fn navigate(&self, route: &str) {
        let mut current = self.route.lock().expect("MemoryRouter mutex poisoned");
        *current = route.to_string();
        self.history.lock().expect("MemoryRouter mutex poisoned").push(route.to_string());
    }

    fn current_route(&self) -> String {
        self.route.lock().expect("MemoryRouter mutex poisoned").clone()
    }
}

/// Result of command execution.
#[derive(Debug)]
pub enum ExecutionResult<M> {
    /// No result (`Command::None` or non-message-producing commands)
    None,
    /// A single message was produced
    Message(M),
    /// Multiple messages were produced
    Messages(Vec<M>),
    /// Command is pending (async)
    Pending,
}

impl<M> ExecutionResult<M> {
    /// Check if the result has no messages.
    #[must_use]
    pub fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }

    /// Check if there are messages.
    #[must_use]
    pub fn has_messages(&self) -> bool {
        matches!(self, Self::Message(_) | Self::Messages(_))
    }

    /// Get messages as a vector.
    pub fn into_messages(self) -> Vec<M> {
        match self {
            Self::None | Self::Pending => vec![],
            Self::Message(m) => vec![m],
            Self::Messages(ms) => ms,
        }
    }
}

/// Command executor configuration.
pub struct ExecutorConfig<R, S> {
    /// Router for navigation commands
    pub router: Arc<R>,
    /// Storage for persistence commands
    pub storage: Arc<S>,
}

impl<R: Router, S: Storage> ExecutorConfig<R, S> {
    /// Create a new executor config.
    pub fn new(router: R, storage: S) -> Self {
        Self {
            router: Arc::new(router),
            storage: Arc::new(storage),
        }
    }
}

/// Command executor for synchronous commands.
///
/// Note: Task commands require async execution and return `ExecutionResult::Pending`.
pub struct CommandExecutor<R, S> {
    config: ExecutorConfig<R, S>,
}

impl<R: Router, S: Storage> CommandExecutor<R, S> {
    /// Create a new command executor.
    pub fn new(config: ExecutorConfig<R, S>) -> Self {
        Self { config }
    }

    /// Execute a command synchronously.
    ///
    /// For async Task commands, this returns `ExecutionResult::Pending`.
    /// Use `execute_blocking` to block on async tasks.
    pub fn execute<M: Send>(&self, command: Command<M>) -> ExecutionResult<M> {
        match command {
            Command::None => ExecutionResult::None,
            Command::Batch(commands) => {
                let mut messages = Vec::new();
                for cmd in commands {
                    match self.execute(cmd) {
                        ExecutionResult::None | ExecutionResult::Pending => {}
                        ExecutionResult::Message(m) => messages.push(m),
                        ExecutionResult::Messages(ms) => messages.extend(ms),
                    }
                }
                if messages.is_empty() {
                    ExecutionResult::None
                } else {
                    ExecutionResult::Messages(messages)
                }
            }
            Command::Task(_) => {
                // Async tasks can't be executed synchronously
                ExecutionResult::Pending
            }
            Command::Navigate { route } => {
                self.config.router.navigate(&route);
                ExecutionResult::None
            }
            Command::SaveState { key } => {
                // SaveState requires the actual state to be passed
                // This is a limitation - we'd need state access
                // For now, just record that we tried to save
                // In practice, the runtime would have state access
                let _ = key;
                ExecutionResult::None
            }
            Command::LoadState { key, on_load } => {
                let data = self.config.storage.load(&key);
                let message = on_load(data);
                ExecutionResult::Message(message)
            }
        }
    }

    /// Get the router.
    pub fn router(&self) -> &R {
        &self.config.router
    }

    /// Get the storage.
    pub fn storage(&self) -> &S {
        &self.config.storage
    }
}

/// Create a default executor with memory-based backends.
#[must_use]
pub fn default_executor() -> CommandExecutor<MemoryRouter, MemoryStorage> {
    CommandExecutor::new(ExecutorConfig::new(
        MemoryRouter::new(),
        MemoryStorage::new(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // MemoryStorage Tests
    // =========================================================================

    #[test]
    fn test_memory_storage_new() {
        let storage = MemoryStorage::new();
        assert!(storage.is_empty());
        assert_eq!(storage.len(), 0);
    }

    #[test]
    fn test_memory_storage_save_load() {
        let storage = MemoryStorage::new();
        storage.save("key1", b"value1");

        assert!(!storage.is_empty());
        assert_eq!(storage.len(), 1);
        assert_eq!(storage.load("key1"), Some(b"value1".to_vec()));
    }

    #[test]
    fn test_memory_storage_load_missing() {
        let storage = MemoryStorage::new();
        assert_eq!(storage.load("nonexistent"), None);
    }

    #[test]
    fn test_memory_storage_contains() {
        let storage = MemoryStorage::new();
        storage.save("exists", b"data");

        assert!(storage.contains("exists"));
        assert!(!storage.contains("missing"));
    }

    #[test]
    fn test_memory_storage_remove() {
        let storage = MemoryStorage::new();
        storage.save("key", b"value");
        assert!(storage.contains("key"));

        storage.remove("key");
        assert!(!storage.contains("key"));
    }

    #[test]
    fn test_memory_storage_clear() {
        let storage = MemoryStorage::new();
        storage.save("a", b"1");
        storage.save("b", b"2");
        assert_eq!(storage.len(), 2);

        storage.clear();
        assert!(storage.is_empty());
    }

    #[test]
    fn test_memory_storage_overwrite() {
        let storage = MemoryStorage::new();
        storage.save("key", b"first");
        storage.save("key", b"second");

        assert_eq!(storage.len(), 1);
        assert_eq!(storage.load("key"), Some(b"second".to_vec()));
    }

    // =========================================================================
    // MemoryRouter Tests
    // =========================================================================

    #[test]
    fn test_memory_router_new() {
        let router = MemoryRouter::new();
        assert_eq!(router.current_route(), "/");
        assert_eq!(router.history_len(), 1);
    }

    #[test]
    fn test_memory_router_navigate() {
        let router = MemoryRouter::new();
        router.navigate("/home");

        assert_eq!(router.current_route(), "/home");
    }

    #[test]
    fn test_memory_router_history() {
        let router = MemoryRouter::new();
        router.navigate("/page1");
        router.navigate("/page2");
        router.navigate("/page3");

        let history = router.history();
        assert_eq!(history, vec!["/", "/page1", "/page2", "/page3"]);
    }

    #[test]
    fn test_memory_router_default() {
        let router = MemoryRouter::default();
        assert_eq!(router.current_route(), "/");
    }

    // =========================================================================
    // ExecutionResult Tests
    // =========================================================================

    #[test]
    fn test_execution_result_none() {
        let result: ExecutionResult<i32> = ExecutionResult::None;
        assert!(result.is_none());
        assert!(!result.has_messages());
    }

    #[test]
    fn test_execution_result_message() {
        let result = ExecutionResult::Message(42);
        assert!(!result.is_none());
        assert!(result.has_messages());
    }

    #[test]
    fn test_execution_result_messages() {
        let result = ExecutionResult::Messages(vec![1, 2, 3]);
        assert!(!result.is_none());
        assert!(result.has_messages());
    }

    #[test]
    fn test_execution_result_pending() {
        let result: ExecutionResult<i32> = ExecutionResult::Pending;
        assert!(!result.is_none());
        assert!(!result.has_messages());
    }

    #[test]
    fn test_execution_result_into_messages_none() {
        let result: ExecutionResult<i32> = ExecutionResult::None;
        assert!(result.into_messages().is_empty());
    }

    #[test]
    fn test_execution_result_into_messages_single() {
        let result = ExecutionResult::Message(42);
        assert_eq!(result.into_messages(), vec![42]);
    }

    #[test]
    fn test_execution_result_into_messages_multiple() {
        let result = ExecutionResult::Messages(vec![1, 2, 3]);
        assert_eq!(result.into_messages(), vec![1, 2, 3]);
    }

    #[test]
    fn test_execution_result_into_messages_pending() {
        let result: ExecutionResult<i32> = ExecutionResult::Pending;
        assert!(result.into_messages().is_empty());
    }

    // =========================================================================
    // CommandExecutor Tests
    // =========================================================================

    #[test]
    fn test_executor_execute_none() {
        let executor = default_executor();
        let result = executor.execute::<()>(Command::None);
        assert!(result.is_none());
    }

    #[test]
    fn test_executor_execute_navigate() {
        let executor = default_executor();
        let result = executor.execute::<()>(Command::Navigate {
            route: "/dashboard".to_string(),
        });

        assert!(result.is_none());
        assert_eq!(executor.router().current_route(), "/dashboard");
    }

    #[test]
    fn test_executor_execute_navigate_multiple() {
        let executor = default_executor();

        executor.execute::<()>(Command::Navigate {
            route: "/page1".to_string(),
        });
        executor.execute::<()>(Command::Navigate {
            route: "/page2".to_string(),
        });

        assert_eq!(executor.router().current_route(), "/page2");
        assert_eq!(executor.router().history_len(), 3); // "/" + "/page1" + "/page2"
    }

    fn load_state_handler(data: Option<Vec<u8>>) -> String {
        data.map_or_else(
            || "not found".to_string(),
            |d| String::from_utf8(d).unwrap(),
        )
    }

    #[test]
    fn test_executor_execute_load_state_found() {
        let executor = default_executor();
        executor.storage().save("my_key", b"stored_data");

        let result = executor.execute(Command::LoadState {
            key: "my_key".to_string(),
            on_load: load_state_handler,
        });

        match result {
            ExecutionResult::Message(msg) => assert_eq!(msg, "stored_data"),
            _ => panic!("Expected Message result"),
        }
    }

    #[test]
    fn test_executor_execute_load_state_not_found() {
        let executor = default_executor();

        let result = executor.execute(Command::LoadState {
            key: "missing_key".to_string(),
            on_load: load_state_handler,
        });

        match result {
            ExecutionResult::Message(msg) => assert_eq!(msg, "not found"),
            _ => panic!("Expected Message result"),
        }
    }

    #[test]
    fn test_executor_execute_batch_empty() {
        let executor = default_executor();
        let result = executor.execute::<()>(Command::Batch(vec![]));
        assert!(result.is_none());
    }

    #[test]
    fn test_executor_execute_batch_navigations() {
        let executor = default_executor();
        let result = executor.execute::<()>(Command::Batch(vec![
            Command::Navigate {
                route: "/a".to_string(),
            },
            Command::Navigate {
                route: "/b".to_string(),
            },
            Command::Navigate {
                route: "/c".to_string(),
            },
        ]));

        assert!(result.is_none());
        assert_eq!(executor.router().current_route(), "/c");
        assert_eq!(executor.router().history_len(), 4);
    }

    fn batch_load_handler(data: Option<Vec<u8>>) -> i32 {
        data.map_or(0, |_| 42)
    }

    #[test]
    fn test_executor_execute_batch_mixed() {
        let executor = default_executor();
        executor.storage().save("key", b"data");

        let result = executor.execute(Command::Batch(vec![
            Command::Navigate {
                route: "/page".to_string(),
            },
            Command::LoadState {
                key: "key".to_string(),
                on_load: batch_load_handler,
            },
        ]));

        match result {
            ExecutionResult::Messages(msgs) => {
                assert_eq!(msgs, vec![42]);
            }
            _ => panic!("Expected Messages result"),
        }
        assert_eq!(executor.router().current_route(), "/page");
    }

    #[test]
    fn test_executor_execute_task_returns_pending() {
        let executor = default_executor();
        let result = executor.execute(Command::task(async { 42 }));

        match result {
            ExecutionResult::Pending => {}
            _ => panic!("Expected Pending result for Task"),
        }
    }

    #[test]
    fn test_executor_execute_save_state() {
        let executor = default_executor();
        let result = executor.execute::<()>(Command::SaveState {
            key: "test".to_string(),
        });

        // SaveState without state access just returns None
        assert!(result.is_none());
    }

    #[test]
    fn test_default_executor() {
        let executor = default_executor();
        assert_eq!(executor.router().current_route(), "/");
        assert!(executor.storage().is_empty());
    }

    // =========================================================================
    // Integration Tests
    // =========================================================================

    #[test]
    fn test_state_update_with_command_execution() {
        use crate::state::{CounterMessage, CounterState, State};

        let executor = default_executor();
        let mut state = CounterState::default();

        // Update state
        let cmd = state.update(CounterMessage::Increment);
        assert_eq!(state.count, 1);

        // Execute command (should be None for CounterState)
        let result = executor.execute(cmd);
        assert!(result.is_none());
    }

    #[test]
    fn test_navigation_state_flow() {
        let executor = default_executor();

        // Simulate app navigation
        executor.execute::<()>(Command::Navigate {
            route: "/login".to_string(),
        });
        assert_eq!(executor.router().current_route(), "/login");

        executor.execute::<()>(Command::Navigate {
            route: "/dashboard".to_string(),
        });
        assert_eq!(executor.router().current_route(), "/dashboard");

        // Check history
        let history = executor.router().history();
        assert_eq!(history, vec!["/", "/login", "/dashboard"]);
    }

    fn serialized_state_handler(data: Option<Vec<u8>>) -> Option<i32> {
        data.and_then(|d| {
            let json = String::from_utf8(d).ok()?;
            // Simple extraction for test
            let count_str = json.split(':').nth(1)?;
            count_str.trim_end_matches('}').parse().ok()
        })
    }

    #[test]
    fn test_load_state_with_serialized_data() {
        let executor = default_executor();

        // Simulate saved state (serialized counter)
        let saved_data = br#"{"count":42}"#;
        executor.storage().save("counter_state", saved_data);

        let result = executor.execute(Command::LoadState {
            key: "counter_state".to_string(),
            on_load: serialized_state_handler,
        });

        match result {
            ExecutionResult::Message(Some(count)) => assert_eq!(count, 42),
            _ => panic!("Expected Message with Some(42)"),
        }
    }
}
