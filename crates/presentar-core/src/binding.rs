//! Interactive state binding for reactive UI.
//!
//! This module provides mechanisms to bind UI widget properties to application
//! state, enabling declarative data flow and automatic UI updates.
//!
//! # Binding Types
//!
//! - `Binding<T>` - Two-way binding for read/write access
//! - `Derived<T>` - Read-only computed value from state
//! - `PropertyPath` - Path to a property in state (e.g., "user.name")
//!
//! # Example
//!
//! ```ignore
//! use presentar_core::binding::{Binding, Derived, PropertyPath};
//!
//! // Create a binding to state.count
//! let count_binding = Binding::new(|| state.count, |v| state.count = v);
//!
//! // Create a derived value
//! let doubled = Derived::new(|| state.count * 2);
//! ```

use serde::{Deserialize, Serialize};
use std::any::Any;
use std::fmt;
use std::sync::{Arc, RwLock};

/// Type alias for subscriber callbacks.
type SubscriberFn<T> = Box<dyn Fn(&T) + Send + Sync>;

/// Type alias for subscribers list.
type Subscribers<T> = Arc<RwLock<Vec<SubscriberFn<T>>>>;

/// A property path for accessing nested state.
///
/// Property paths use dot notation: "user.profile.name"
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PropertyPath {
    segments: Vec<String>,
}

impl PropertyPath {
    /// Create a new property path from a string.
    #[must_use]
    pub fn new(path: &str) -> Self {
        let segments = path
            .split('.')
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect();
        Self { segments }
    }

    /// Create an empty root path.
    #[must_use]
    pub const fn root() -> Self {
        Self {
            segments: Vec::new(),
        }
    }

    /// Get path segments.
    #[must_use]
    pub fn segments(&self) -> &[String] {
        &self.segments
    }

    /// Check if path is empty (root).
    #[must_use]
    pub fn is_root(&self) -> bool {
        self.segments.is_empty()
    }

    /// Get the number of segments.
    #[must_use]
    pub fn len(&self) -> usize {
        self.segments.len()
    }

    /// Check if path is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.segments.is_empty()
    }

    /// Append a segment to the path.
    #[must_use]
    pub fn join(&self, segment: &str) -> Self {
        let mut segments = self.segments.clone();
        segments.push(segment.to_string());
        Self { segments }
    }

    /// Get the parent path.
    #[must_use]
    pub fn parent(&self) -> Option<Self> {
        if self.segments.is_empty() {
            None
        } else {
            let mut segments = self.segments.clone();
            segments.pop();
            Some(Self { segments })
        }
    }

    /// Get the last segment (leaf name).
    #[must_use]
    pub fn leaf(&self) -> Option<&str> {
        self.segments.last().map(String::as_str)
    }

    /// Convert to string representation.
    #[must_use]
    pub fn to_string_path(&self) -> String {
        self.segments.join(".")
    }
}

impl fmt::Display for PropertyPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_string_path())
    }
}

impl From<&str> for PropertyPath {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

/// Binding direction for property connections.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BindingDirection {
    /// One-way binding: state → widget
    #[default]
    OneWay,
    /// Two-way binding: state ↔ widget
    TwoWay,
    /// One-time binding: state → widget (initial only)
    OneTime,
}

/// A binding configuration for connecting state to widget properties.
#[derive(Debug, Clone)]
pub struct BindingConfig {
    /// Source path in state
    pub source: PropertyPath,
    /// Target property on widget
    pub target: String,
    /// Binding direction
    pub direction: BindingDirection,
    /// Optional transform function name
    pub transform: Option<String>,
    /// Optional fallback value (as string)
    pub fallback: Option<String>,
}

impl BindingConfig {
    /// Create a new one-way binding.
    #[must_use]
    pub fn one_way(source: impl Into<PropertyPath>, target: impl Into<String>) -> Self {
        Self {
            source: source.into(),
            target: target.into(),
            direction: BindingDirection::OneWay,
            transform: None,
            fallback: None,
        }
    }

    /// Create a new two-way binding.
    #[must_use]
    pub fn two_way(source: impl Into<PropertyPath>, target: impl Into<String>) -> Self {
        Self {
            source: source.into(),
            target: target.into(),
            direction: BindingDirection::TwoWay,
            transform: None,
            fallback: None,
        }
    }

    /// Set a transform function.
    #[must_use]
    pub fn transform(mut self, name: impl Into<String>) -> Self {
        self.transform = Some(name.into());
        self
    }

    /// Set a fallback value.
    #[must_use]
    pub fn fallback(mut self, value: impl Into<String>) -> Self {
        self.fallback = Some(value.into());
        self
    }
}

/// Trait for types that can be bound to state.
pub trait Bindable: Any + Send + Sync {
    /// Get bindings for this widget.
    fn bindings(&self) -> Vec<BindingConfig>;

    /// Set bindings for this widget.
    fn set_bindings(&mut self, bindings: Vec<BindingConfig>);

    /// Apply a binding value update.
    fn apply_binding(&mut self, target: &str, value: &dyn Any) -> bool;

    /// Get the current value for a binding target.
    fn get_binding_value(&self, target: &str) -> Option<Box<dyn Any + Send>>;
}

/// A reactive cell that holds a value and notifies on changes.
pub struct ReactiveCell<T> {
    value: Arc<RwLock<T>>,
    subscribers: Subscribers<T>,
}

impl<T: Clone + Send + Sync + 'static> ReactiveCell<T> {
    /// Create a new reactive cell with an initial value.
    pub fn new(value: T) -> Self {
        Self {
            value: Arc::new(RwLock::new(value)),
            subscribers: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Get the current value.
    pub fn get(&self) -> T {
        self.value.read().expect("ReactiveCell lock poisoned").clone()
    }

    /// Set a new value, notifying subscribers.
    pub fn set(&self, value: T) {
        {
            let mut guard = self.value.write().expect("ReactiveCell lock poisoned");
            *guard = value;
        }
        self.notify();
    }

    /// Update the value using a function.
    pub fn update<F>(&self, f: F)
    where
        F: FnOnce(&mut T),
    {
        {
            let mut guard = self.value.write().expect("ReactiveCell lock poisoned");
            f(&mut guard);
        }
        self.notify();
    }

    /// Subscribe to value changes.
    pub fn subscribe<F>(&self, callback: F)
    where
        F: Fn(&T) + Send + Sync + 'static,
    {
        self.subscribers.write().expect("ReactiveCell lock poisoned").push(Box::new(callback));
    }

    fn notify(&self) {
        let value = self.value.read().expect("ReactiveCell lock poisoned");
        let subscribers = self.subscribers.read().expect("ReactiveCell lock poisoned");
        for sub in subscribers.iter() {
            sub(&value);
        }
    }
}

impl<T: Clone + Send + Sync> Clone for ReactiveCell<T> {
    fn clone(&self) -> Self {
        Self {
            value: self.value.clone(),
            subscribers: Arc::new(RwLock::new(Vec::new())), // Don't clone subscribers
        }
    }
}

impl<T: Clone + Send + Sync + Default + 'static> Default for ReactiveCell<T> {
    fn default() -> Self {
        Self::new(T::default())
    }
}

impl<T: Clone + Send + Sync + fmt::Debug + 'static> fmt::Debug for ReactiveCell<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ReactiveCell")
            .field("value", &*self.value.read().expect("ReactiveCell lock poisoned"))
            .finish_non_exhaustive()
    }
}

/// A computed value derived from other reactive sources.
pub struct Computed<T> {
    #[allow(dead_code)]
    compute: Box<dyn Fn() -> T + Send + Sync>,
    cached: Arc<RwLock<Option<T>>>,
    dirty: Arc<RwLock<bool>>,
}

impl<T: Clone + Send + Sync + 'static> Computed<T> {
    /// Create a new computed value.
    pub fn new<F>(compute: F) -> Self
    where
        F: Fn() -> T + Send + Sync + 'static,
    {
        Self {
            compute: Box::new(compute),
            cached: Arc::new(RwLock::new(None)),
            dirty: Arc::new(RwLock::new(true)),
        }
    }

    /// Get the computed value (caches result).
    pub fn get(&self) -> T {
        let dirty = *self.dirty.read().expect("Computed lock poisoned");
        if dirty {
            let value = (self.compute)();
            *self.cached.write().expect("Computed lock poisoned") = Some(value.clone());
            *self.dirty.write().expect("Computed lock poisoned") = false;
            value
        } else {
            self.cached.read().expect("Computed lock poisoned")
                .clone()
                .expect("Computed cache should contain value when not dirty")
        }
    }

    /// Mark the computed value as dirty (needs recomputation).
    pub fn invalidate(&self) {
        *self.dirty.write().expect("Computed lock poisoned") = true;
    }
}

impl<T: Clone + Send + Sync + fmt::Debug + 'static> fmt::Debug for Computed<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Computed")
            .field("cached", &*self.cached.read().expect("Computed lock poisoned"))
            .field("dirty", &*self.dirty.read().expect("Computed lock poisoned"))
            .finish_non_exhaustive()
    }
}

/// A binding expression that can be evaluated against state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BindingExpression {
    /// Expression string (e.g., "{{ user.name }}" or "{{ count * 2 }}")
    pub expression: String,
    /// Parsed dependencies (property paths used)
    pub dependencies: Vec<PropertyPath>,
}

impl BindingExpression {
    /// Create a new binding expression.
    #[must_use]
    pub fn new(expression: impl Into<String>) -> Self {
        let expression = expression.into();
        let dependencies = Self::parse_dependencies(&expression);
        Self {
            expression,
            dependencies,
        }
    }

    /// Create a simple property binding.
    #[must_use]
    pub fn property(path: impl Into<PropertyPath>) -> Self {
        let path: PropertyPath = path.into();
        let expression = format!("{{{{ {} }}}}", path.to_string_path());
        Self {
            expression,
            dependencies: vec![path],
        }
    }

    /// Check if this is a simple property binding (no transforms).
    #[must_use]
    pub fn is_simple_property(&self) -> bool {
        self.dependencies.len() == 1
            && self.expression.trim().starts_with("{{")
            && self.expression.trim().ends_with("}}")
    }

    /// Get the property path if this is a simple binding.
    #[must_use]
    pub fn as_property(&self) -> Option<&PropertyPath> {
        if self.is_simple_property() {
            self.dependencies.first()
        } else {
            None
        }
    }

    fn parse_dependencies(expression: &str) -> Vec<PropertyPath> {
        let mut deps = Vec::new();
        let mut in_binding = false;
        let mut current = String::new();

        let chars: Vec<char> = expression.chars().collect();
        let mut i = 0;

        while i < chars.len() {
            if i + 1 < chars.len() && chars[i] == '{' && chars[i + 1] == '{' {
                in_binding = true;
                i += 2;
                continue;
            }

            if i + 1 < chars.len() && chars[i] == '}' && chars[i + 1] == '}' {
                if !current.is_empty() {
                    // Extract property path from current (may have transforms)
                    let path_str = current.split('|').next().unwrap_or("").trim();
                    if !path_str.is_empty() && !path_str.contains(|c: char| c.is_whitespace()) {
                        deps.push(PropertyPath::new(path_str));
                    }
                    current.clear();
                }
                in_binding = false;
                i += 2;
                continue;
            }

            if in_binding {
                current.push(chars[i]);
            }

            i += 1;
        }

        deps
    }
}

/// Event binding that maps widget events to state messages.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventBinding {
    /// Widget event name (e.g., "click", "change", "submit")
    pub event: String,
    /// Action to dispatch
    pub action: ActionBinding,
}

/// Action binding for state updates.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ActionBinding {
    /// Set a property to a value
    SetProperty {
        /// Property path
        path: PropertyPath,
        /// Value expression
        value: String,
    },
    /// Toggle a boolean property
    ToggleProperty {
        /// Property path
        path: PropertyPath,
    },
    /// Increment a numeric property
    IncrementProperty {
        /// Property path
        path: PropertyPath,
        /// Amount to increment (default 1)
        amount: Option<f64>,
    },
    /// Navigate to a route
    Navigate {
        /// Route path
        route: String,
    },
    /// Dispatch a custom message
    Dispatch {
        /// Message type/name
        message: String,
        /// Optional payload
        payload: Option<String>,
    },
    /// Execute multiple actions
    Batch {
        /// Actions to execute
        actions: Vec<ActionBinding>,
    },
}

impl EventBinding {
    /// Create a new event binding.
    #[must_use]
    pub fn new(event: impl Into<String>, action: ActionBinding) -> Self {
        Self {
            event: event.into(),
            action,
        }
    }

    /// Create a click event binding.
    #[must_use]
    pub fn on_click(action: ActionBinding) -> Self {
        Self::new("click", action)
    }

    /// Create a change event binding.
    #[must_use]
    pub fn on_change(action: ActionBinding) -> Self {
        Self::new("change", action)
    }
}

impl ActionBinding {
    /// Create a set property action.
    #[must_use]
    pub fn set(path: impl Into<PropertyPath>, value: impl Into<String>) -> Self {
        Self::SetProperty {
            path: path.into(),
            value: value.into(),
        }
    }

    /// Create a toggle action.
    #[must_use]
    pub fn toggle(path: impl Into<PropertyPath>) -> Self {
        Self::ToggleProperty { path: path.into() }
    }

    /// Create an increment action.
    #[must_use]
    pub fn increment(path: impl Into<PropertyPath>) -> Self {
        Self::IncrementProperty {
            path: path.into(),
            amount: None,
        }
    }

    /// Create an increment by amount action.
    #[must_use]
    pub fn increment_by(path: impl Into<PropertyPath>, amount: f64) -> Self {
        Self::IncrementProperty {
            path: path.into(),
            amount: Some(amount),
        }
    }

    /// Create a navigate action.
    #[must_use]
    pub fn navigate(route: impl Into<String>) -> Self {
        Self::Navigate {
            route: route.into(),
        }
    }

    /// Create a dispatch action.
    #[must_use]
    pub fn dispatch(message: impl Into<String>) -> Self {
        Self::Dispatch {
            message: message.into(),
            payload: None,
        }
    }

    /// Create a dispatch with payload action.
    #[must_use]
    pub fn dispatch_with(message: impl Into<String>, payload: impl Into<String>) -> Self {
        Self::Dispatch {
            message: message.into(),
            payload: Some(payload.into()),
        }
    }

    /// Create a batch of actions.
    #[must_use]
    pub fn batch(actions: impl IntoIterator<Item = Self>) -> Self {
        Self::Batch {
            actions: actions.into_iter().collect(),
        }
    }
}

// =============================================================================
// BindingManager - Orchestrates State-Widget Bindings
// =============================================================================

/// Manages bindings between application state and widget properties.
///
/// The `BindingManager` provides:
/// - Registration of two-way bindings
/// - Automatic propagation of state changes to widgets
/// - Handling of widget changes back to state
/// - Debouncing support for frequent updates
#[derive(Debug, Default)]
pub struct BindingManager {
    /// Active bindings
    bindings: Vec<ActiveBinding>,
    /// Whether to debounce updates
    debounce_ms: Option<u32>,
    /// Pending updates queue
    pending_updates: Vec<PendingUpdate>,
}

/// An active binding between state and widget.
#[derive(Debug, Clone)]
pub struct ActiveBinding {
    /// Unique binding ID
    pub id: BindingId,
    /// Widget ID
    pub widget_id: String,
    /// Binding configuration
    pub config: BindingConfig,
    /// Current state value (as string for simplicity)
    pub current_value: Option<String>,
    /// Whether binding is active
    pub active: bool,
}

/// Unique binding identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct BindingId(pub u64);

/// A pending update to be applied.
#[derive(Debug, Clone)]
pub struct PendingUpdate {
    /// Source (widget or state)
    pub source: UpdateSource,
    /// Property path
    pub path: PropertyPath,
    /// New value as string
    pub value: String,
    /// Timestamp
    pub timestamp: u64,
}

/// Source of a binding update.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateSource {
    /// Update from state
    State,
    /// Update from widget
    Widget,
}

impl BindingManager {
    /// Create a new binding manager.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set debounce delay in milliseconds.
    #[must_use]
    pub fn with_debounce(mut self, ms: u32) -> Self {
        self.debounce_ms = Some(ms);
        self
    }

    /// Register a binding between state and widget.
    pub fn register(&mut self, widget_id: impl Into<String>, config: BindingConfig) -> BindingId {
        let id = BindingId(self.bindings.len() as u64);
        self.bindings.push(ActiveBinding {
            id,
            widget_id: widget_id.into(),
            config,
            current_value: None,
            active: true,
        });
        id
    }

    /// Unregister a binding.
    pub fn unregister(&mut self, id: BindingId) {
        if let Some(binding) = self.bindings.iter_mut().find(|b| b.id == id) {
            binding.active = false;
        }
    }

    /// Get bindings for a widget.
    #[must_use]
    pub fn bindings_for_widget(&self, widget_id: &str) -> Vec<&ActiveBinding> {
        self.bindings
            .iter()
            .filter(|b| b.active && b.widget_id == widget_id)
            .collect()
    }

    /// Get bindings for a state path.
    #[must_use]
    pub fn bindings_for_path(&self, path: &PropertyPath) -> Vec<&ActiveBinding> {
        self.bindings
            .iter()
            .filter(|b| b.active && &b.config.source == path)
            .collect()
    }

    /// Handle state change, propagate to widgets.
    pub fn on_state_change(&mut self, path: &PropertyPath, value: &str) -> Vec<WidgetUpdate> {
        let mut updates = Vec::new();

        for binding in &mut self.bindings {
            if !binding.active {
                continue;
            }

            // Check if this path affects this binding
            if &binding.config.source == path
                || path.to_string_path().starts_with(&binding.config.source.to_string_path())
            {
                binding.current_value = Some(value.to_string());

                updates.push(WidgetUpdate {
                    widget_id: binding.widget_id.clone(),
                    property: binding.config.target.clone(),
                    value: value.to_string(),
                });
            }
        }

        updates
    }

    /// Handle widget change, propagate to state.
    pub fn on_widget_change(
        &mut self,
        widget_id: &str,
        property: &str,
        value: &str,
    ) -> Vec<StateUpdate> {
        let mut updates = Vec::new();

        for binding in &self.bindings {
            if !binding.active {
                continue;
            }

            // Only two-way bindings propagate back to state
            if binding.config.direction != BindingDirection::TwoWay {
                continue;
            }

            if binding.widget_id == widget_id && binding.config.target == property {
                updates.push(StateUpdate {
                    path: binding.config.source.clone(),
                    value: value.to_string(),
                });
            }
        }

        updates
    }

    /// Queue an update (for debouncing).
    pub fn queue_update(&mut self, source: UpdateSource, path: PropertyPath, value: String) {
        self.pending_updates.push(PendingUpdate {
            source,
            path,
            value,
            timestamp: 0, // Would be set to actual timestamp
        });
    }

    /// Flush pending updates.
    pub fn flush(&mut self) -> (Vec<WidgetUpdate>, Vec<StateUpdate>) {
        let mut widget_updates = Vec::new();
        let mut state_updates = Vec::new();

        // Drain into separate Vec to avoid borrow issues
        let updates: Vec<PendingUpdate> = self.pending_updates.drain(..).collect();

        for update in updates {
            match update.source {
                UpdateSource::State => {
                    widget_updates.extend(self.on_state_change(&update.path, &update.value));
                }
                UpdateSource::Widget => {
                    // For widget updates, we'd need widget_id context
                    state_updates.push(StateUpdate {
                        path: update.path,
                        value: update.value,
                    });
                }
            }
        }

        (widget_updates, state_updates)
    }

    /// Get number of active bindings.
    #[must_use]
    pub fn active_count(&self) -> usize {
        self.bindings.iter().filter(|b| b.active).count()
    }

    /// Clear all bindings.
    pub fn clear(&mut self) {
        self.bindings.clear();
        self.pending_updates.clear();
    }
}

/// Update to apply to a widget.
#[derive(Debug, Clone)]
pub struct WidgetUpdate {
    /// Target widget ID
    pub widget_id: String,
    /// Property to update
    pub property: String,
    /// New value
    pub value: String,
}

/// Update to apply to state.
#[derive(Debug, Clone)]
pub struct StateUpdate {
    /// Property path
    pub path: PropertyPath,
    /// New value
    pub value: String,
}

// =============================================================================
// ValueConverter - Type Conversion for Bindings
// =============================================================================

/// Converts values between different types for binding.
pub trait ValueConverter: Send + Sync {
    /// Convert from source type to target type.
    fn convert(&self, value: &str) -> Result<String, ConversionError>;

    /// Convert back from target type to source type.
    fn convert_back(&self, value: &str) -> Result<String, ConversionError>;
}

/// Error during value conversion.
#[derive(Debug, Clone)]
pub struct ConversionError {
    /// Error message
    pub message: String,
}

impl fmt::Display for ConversionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "conversion error: {}", self.message)
    }
}

impl std::error::Error for ConversionError {}

/// Identity converter (no conversion).
#[derive(Debug, Default)]
pub struct IdentityConverter;

impl ValueConverter for IdentityConverter {
    fn convert(&self, value: &str) -> Result<String, ConversionError> {
        Ok(value.to_string())
    }

    fn convert_back(&self, value: &str) -> Result<String, ConversionError> {
        Ok(value.to_string())
    }
}

/// Boolean to string converter.
#[derive(Debug, Default)]
pub struct BoolToStringConverter {
    /// String for true value
    pub true_string: String,
    /// String for false value
    pub false_string: String,
}

impl BoolToStringConverter {
    /// Create with "true"/"false" strings.
    #[must_use]
    pub fn new() -> Self {
        Self {
            true_string: "true".to_string(),
            false_string: "false".to_string(),
        }
    }

    /// Create with custom strings.
    #[must_use]
    pub fn with_strings(true_str: impl Into<String>, false_str: impl Into<String>) -> Self {
        Self {
            true_string: true_str.into(),
            false_string: false_str.into(),
        }
    }
}

impl ValueConverter for BoolToStringConverter {
    fn convert(&self, value: &str) -> Result<String, ConversionError> {
        match value {
            "true" | "1" | "yes" => Ok(self.true_string.clone()),
            "false" | "0" | "no" => Ok(self.false_string.clone()),
            _ => Err(ConversionError {
                message: format!("cannot convert '{value}' to bool"),
            }),
        }
    }

    fn convert_back(&self, value: &str) -> Result<String, ConversionError> {
        if value == self.true_string {
            Ok("true".to_string())
        } else if value == self.false_string {
            Ok("false".to_string())
        } else {
            Err(ConversionError {
                message: format!("cannot convert '{value}' back to bool"),
            })
        }
    }
}

/// Number formatter converter.
#[derive(Debug, Default)]
pub struct NumberFormatConverter {
    /// Decimal places
    pub decimals: usize,
    /// Prefix (e.g., "$")
    pub prefix: String,
    /// Suffix (e.g., "%")
    pub suffix: String,
}

impl NumberFormatConverter {
    /// Create default formatter.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set decimal places.
    #[must_use]
    pub fn decimals(mut self, places: usize) -> Self {
        self.decimals = places;
        self
    }

    /// Set prefix.
    #[must_use]
    pub fn prefix(mut self, prefix: impl Into<String>) -> Self {
        self.prefix = prefix.into();
        self
    }

    /// Set suffix.
    #[must_use]
    pub fn suffix(mut self, suffix: impl Into<String>) -> Self {
        self.suffix = suffix.into();
        self
    }
}

impl ValueConverter for NumberFormatConverter {
    fn convert(&self, value: &str) -> Result<String, ConversionError> {
        let num: f64 = value.parse().map_err(|_| ConversionError {
            message: format!("cannot parse '{value}' as number"),
        })?;

        let formatted = format!("{:.prec$}", num, prec = self.decimals);
        Ok(format!("{}{}{}", self.prefix, formatted, self.suffix))
    }

    fn convert_back(&self, value: &str) -> Result<String, ConversionError> {
        // Strip prefix and suffix
        let stripped = value
            .strip_prefix(&self.prefix)
            .unwrap_or(value)
            .strip_suffix(&self.suffix)
            .unwrap_or(value)
            .trim();

        // Validate it's a number
        let _: f64 = stripped.parse().map_err(|_| ConversionError {
            message: format!("cannot parse '{stripped}' as number"),
        })?;

        Ok(stripped.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // PropertyPath Tests
    // =========================================================================

    #[test]
    fn test_property_path_new() {
        let path = PropertyPath::new("user.profile.name");
        assert_eq!(path.segments(), &["user", "profile", "name"]);
    }

    #[test]
    fn test_property_path_root() {
        let path = PropertyPath::root();
        assert!(path.is_root());
        assert!(path.is_empty());
    }

    #[test]
    fn test_property_path_len() {
        let path = PropertyPath::new("a.b.c");
        assert_eq!(path.len(), 3);
    }

    #[test]
    fn test_property_path_join() {
        let path = PropertyPath::new("user");
        let joined = path.join("name");
        assert_eq!(joined.to_string_path(), "user.name");
    }

    #[test]
    fn test_property_path_parent() {
        let path = PropertyPath::new("user.profile.name");
        let parent = path.parent().unwrap();
        assert_eq!(parent.to_string_path(), "user.profile");
    }

    #[test]
    fn test_property_path_leaf() {
        let path = PropertyPath::new("user.profile.name");
        assert_eq!(path.leaf(), Some("name"));
    }

    #[test]
    fn test_property_path_display() {
        let path = PropertyPath::new("a.b.c");
        assert_eq!(format!("{path}"), "a.b.c");
    }

    #[test]
    fn test_property_path_from_str() {
        let path: PropertyPath = "user.name".into();
        assert_eq!(path.segments(), &["user", "name"]);
    }

    // =========================================================================
    // BindingConfig Tests
    // =========================================================================

    #[test]
    fn test_binding_config_one_way() {
        let config = BindingConfig::one_way("count", "value");
        assert_eq!(config.source.to_string_path(), "count");
        assert_eq!(config.target, "value");
        assert_eq!(config.direction, BindingDirection::OneWay);
    }

    #[test]
    fn test_binding_config_two_way() {
        let config = BindingConfig::two_way("user.name", "text");
        assert_eq!(config.direction, BindingDirection::TwoWay);
    }

    #[test]
    fn test_binding_config_transform() {
        let config = BindingConfig::one_way("count", "label").transform("toString");
        assert_eq!(config.transform, Some("toString".to_string()));
    }

    #[test]
    fn test_binding_config_fallback() {
        let config = BindingConfig::one_way("user.name", "text").fallback("Anonymous");
        assert_eq!(config.fallback, Some("Anonymous".to_string()));
    }

    // =========================================================================
    // ReactiveCell Tests
    // =========================================================================

    #[test]
    fn test_reactive_cell_new() {
        let cell = ReactiveCell::new(42);
        assert_eq!(cell.get(), 42);
    }

    #[test]
    fn test_reactive_cell_set() {
        let cell = ReactiveCell::new(0);
        cell.set(100);
        assert_eq!(cell.get(), 100);
    }

    #[test]
    fn test_reactive_cell_update() {
        let cell = ReactiveCell::new(10);
        cell.update(|v| *v *= 2);
        assert_eq!(cell.get(), 20);
    }

    #[test]
    fn test_reactive_cell_subscribe() {
        use std::sync::atomic::{AtomicI32, Ordering};

        let cell = ReactiveCell::new(0);
        let count = Arc::new(AtomicI32::new(0));
        let count_clone = count.clone();

        cell.subscribe(move |v| {
            count_clone.store(*v, Ordering::SeqCst);
        });

        cell.set(42);
        assert_eq!(count.load(Ordering::SeqCst), 42);
    }

    #[test]
    fn test_reactive_cell_default() {
        let cell: ReactiveCell<i32> = ReactiveCell::default();
        assert_eq!(cell.get(), 0);
    }

    #[test]
    fn test_reactive_cell_clone() {
        let cell1 = ReactiveCell::new(10);
        let cell2 = cell1.clone();

        cell1.set(20);
        assert_eq!(cell2.get(), 20); // Shares same underlying value
    }

    // =========================================================================
    // Computed Tests
    // =========================================================================

    #[test]
    fn test_computed_new() {
        let computed = Computed::new(|| 42);
        assert_eq!(computed.get(), 42);
    }

    #[test]
    fn test_computed_caches() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        let call_count = Arc::new(AtomicUsize::new(0));
        let call_count_clone = call_count.clone();

        let computed = Computed::new(move || {
            call_count_clone.fetch_add(1, Ordering::SeqCst);
            42
        });

        computed.get();
        computed.get();
        computed.get();

        assert_eq!(call_count.load(Ordering::SeqCst), 1); // Only computed once
    }

    #[test]
    fn test_computed_invalidate() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        let call_count = Arc::new(AtomicUsize::new(0));
        let call_count_clone = call_count.clone();

        let computed = Computed::new(move || {
            call_count_clone.fetch_add(1, Ordering::SeqCst);
            42
        });

        computed.get();
        computed.invalidate();
        computed.get();

        assert_eq!(call_count.load(Ordering::SeqCst), 2); // Computed twice
    }

    // =========================================================================
    // BindingExpression Tests
    // =========================================================================

    #[test]
    fn test_binding_expression_new() {
        let expr = BindingExpression::new("{{ user.name }}");
        assert_eq!(expr.dependencies.len(), 1);
        assert_eq!(expr.dependencies[0].to_string_path(), "user.name");
    }

    #[test]
    fn test_binding_expression_property() {
        let expr = BindingExpression::property("count");
        assert!(expr.is_simple_property());
        assert_eq!(expr.as_property().unwrap().to_string_path(), "count");
    }

    #[test]
    fn test_binding_expression_with_transform() {
        let expr = BindingExpression::new("{{ count | format }}");
        assert_eq!(expr.dependencies[0].to_string_path(), "count");
    }

    #[test]
    fn test_binding_expression_multiple_deps() {
        let expr = BindingExpression::new("{{ first }} and {{ second }}");
        assert_eq!(expr.dependencies.len(), 2);
    }

    // =========================================================================
    // EventBinding Tests
    // =========================================================================

    #[test]
    fn test_event_binding_new() {
        let binding = EventBinding::new("click", ActionBinding::toggle("visible"));
        assert_eq!(binding.event, "click");
    }

    #[test]
    fn test_event_binding_on_click() {
        let binding = EventBinding::on_click(ActionBinding::dispatch("submit"));
        assert_eq!(binding.event, "click");
    }

    #[test]
    fn test_event_binding_on_change() {
        let binding = EventBinding::on_change(ActionBinding::set("value", "new"));
        assert_eq!(binding.event, "change");
    }

    // =========================================================================
    // ActionBinding Tests
    // =========================================================================

    #[test]
    fn test_action_binding_set() {
        let action = ActionBinding::set("user.name", "Alice");
        if let ActionBinding::SetProperty { path, value } = action {
            assert_eq!(path.to_string_path(), "user.name");
            assert_eq!(value, "Alice");
        } else {
            panic!("Expected SetProperty");
        }
    }

    #[test]
    fn test_action_binding_toggle() {
        let action = ActionBinding::toggle("visible");
        if let ActionBinding::ToggleProperty { path } = action {
            assert_eq!(path.to_string_path(), "visible");
        } else {
            panic!("Expected ToggleProperty");
        }
    }

    #[test]
    fn test_action_binding_increment() {
        let action = ActionBinding::increment("count");
        if let ActionBinding::IncrementProperty { path, amount } = action {
            assert_eq!(path.to_string_path(), "count");
            assert!(amount.is_none());
        } else {
            panic!("Expected IncrementProperty");
        }
    }

    #[test]
    fn test_action_binding_increment_by() {
        let action = ActionBinding::increment_by("score", 10.0);
        if let ActionBinding::IncrementProperty { amount, .. } = action {
            assert_eq!(amount, Some(10.0));
        } else {
            panic!("Expected IncrementProperty");
        }
    }

    #[test]
    fn test_action_binding_navigate() {
        let action = ActionBinding::navigate("/home");
        if let ActionBinding::Navigate { route } = action {
            assert_eq!(route, "/home");
        } else {
            panic!("Expected Navigate");
        }
    }

    #[test]
    fn test_action_binding_dispatch() {
        let action = ActionBinding::dispatch("submit");
        if let ActionBinding::Dispatch { message, payload } = action {
            assert_eq!(message, "submit");
            assert!(payload.is_none());
        } else {
            panic!("Expected Dispatch");
        }
    }

    #[test]
    fn test_action_binding_dispatch_with() {
        let action = ActionBinding::dispatch_with("submit", "form_data");
        if let ActionBinding::Dispatch { message, payload } = action {
            assert_eq!(message, "submit");
            assert_eq!(payload, Some("form_data".to_string()));
        } else {
            panic!("Expected Dispatch");
        }
    }

    #[test]
    fn test_action_binding_batch() {
        let action = ActionBinding::batch([
            ActionBinding::increment("count"),
            ActionBinding::navigate("/next"),
        ]);
        if let ActionBinding::Batch { actions } = action {
            assert_eq!(actions.len(), 2);
        } else {
            panic!("Expected Batch");
        }
    }

    // =========================================================================
    // BindingManager Tests
    // =========================================================================

    #[test]
    fn test_binding_manager_new() {
        let manager = BindingManager::new();
        assert_eq!(manager.active_count(), 0);
    }

    #[test]
    fn test_binding_manager_register() {
        let mut manager = BindingManager::new();
        let id = manager.register("widget1", BindingConfig::one_way("count", "text"));
        assert_eq!(id.0, 0);
        assert_eq!(manager.active_count(), 1);
    }

    #[test]
    fn test_binding_manager_unregister() {
        let mut manager = BindingManager::new();
        let id = manager.register("widget1", BindingConfig::one_way("count", "text"));
        manager.unregister(id);
        assert_eq!(manager.active_count(), 0);
    }

    #[test]
    fn test_binding_manager_bindings_for_widget() {
        let mut manager = BindingManager::new();
        manager.register("widget1", BindingConfig::one_way("count", "text"));
        manager.register("widget1", BindingConfig::one_way("name", "label"));
        manager.register("widget2", BindingConfig::one_way("other", "value"));

        let bindings = manager.bindings_for_widget("widget1");
        assert_eq!(bindings.len(), 2);
    }

    #[test]
    fn test_binding_manager_bindings_for_path() {
        let mut manager = BindingManager::new();
        manager.register("widget1", BindingConfig::one_way("user.name", "text"));
        manager.register("widget2", BindingConfig::one_way("user.name", "label"));

        let path = PropertyPath::new("user.name");
        let bindings = manager.bindings_for_path(&path);
        assert_eq!(bindings.len(), 2);
    }

    #[test]
    fn test_binding_manager_on_state_change() {
        let mut manager = BindingManager::new();
        manager.register("widget1", BindingConfig::one_way("count", "text"));
        manager.register("widget2", BindingConfig::one_way("count", "label"));

        let path = PropertyPath::new("count");
        let updates = manager.on_state_change(&path, "42");

        assert_eq!(updates.len(), 2);
        assert!(updates.iter().any(|u| u.widget_id == "widget1"));
        assert!(updates.iter().any(|u| u.widget_id == "widget2"));
    }

    #[test]
    fn test_binding_manager_on_widget_change_two_way() {
        let mut manager = BindingManager::new();
        manager.register("input1", BindingConfig::two_way("user.name", "value"));

        let updates = manager.on_widget_change("input1", "value", "Alice");

        assert_eq!(updates.len(), 1);
        assert_eq!(updates[0].path.to_string_path(), "user.name");
        assert_eq!(updates[0].value, "Alice");
    }

    #[test]
    fn test_binding_manager_on_widget_change_one_way_no_propagate() {
        let mut manager = BindingManager::new();
        manager.register("label1", BindingConfig::one_way("count", "text"));

        let updates = manager.on_widget_change("label1", "text", "new value");

        assert!(updates.is_empty()); // One-way doesn't propagate back
    }

    #[test]
    fn test_binding_manager_with_debounce() {
        let manager = BindingManager::new().with_debounce(100);
        assert_eq!(manager.debounce_ms, Some(100));
    }

    #[test]
    fn test_binding_manager_queue_and_flush() {
        let mut manager = BindingManager::new();
        manager.register("widget1", BindingConfig::one_way("count", "text"));

        manager.queue_update(
            UpdateSource::State,
            PropertyPath::new("count"),
            "42".to_string(),
        );

        let (widget_updates, _) = manager.flush();
        assert_eq!(widget_updates.len(), 1);
    }

    #[test]
    fn test_binding_manager_clear() {
        let mut manager = BindingManager::new();
        manager.register("w1", BindingConfig::one_way("a", "b"));
        manager.register("w2", BindingConfig::one_way("c", "d"));
        manager.clear();
        assert_eq!(manager.active_count(), 0);
    }

    // =========================================================================
    // ValueConverter Tests
    // =========================================================================

    #[test]
    fn test_identity_converter() {
        let converter = IdentityConverter;
        assert_eq!(converter.convert("hello").unwrap(), "hello");
        assert_eq!(converter.convert_back("world").unwrap(), "world");
    }

    #[test]
    fn test_bool_to_string_converter() {
        let converter = BoolToStringConverter::new();
        assert_eq!(converter.convert("true").unwrap(), "true");
        assert_eq!(converter.convert("false").unwrap(), "false");
        assert_eq!(converter.convert("1").unwrap(), "true");
        assert_eq!(converter.convert("0").unwrap(), "false");
    }

    #[test]
    fn test_bool_to_string_converter_custom() {
        let converter = BoolToStringConverter::with_strings("Yes", "No");
        assert_eq!(converter.convert("true").unwrap(), "Yes");
        assert_eq!(converter.convert("false").unwrap(), "No");
        assert_eq!(converter.convert_back("Yes").unwrap(), "true");
        assert_eq!(converter.convert_back("No").unwrap(), "false");
    }

    #[test]
    fn test_bool_to_string_converter_error() {
        let converter = BoolToStringConverter::new();
        assert!(converter.convert("invalid").is_err());
    }

    #[test]
    fn test_number_format_converter() {
        let converter = NumberFormatConverter::new().decimals(2);
        assert_eq!(converter.convert("42").unwrap(), "42.00");
        assert_eq!(converter.convert("3.14159").unwrap(), "3.14");
    }

    #[test]
    fn test_number_format_converter_with_prefix_suffix() {
        let converter = NumberFormatConverter::new()
            .decimals(2)
            .prefix("$")
            .suffix(" USD");
        assert_eq!(converter.convert("100").unwrap(), "$100.00 USD");
        assert_eq!(converter.convert_back("$100.00 USD").unwrap(), "100.00");
    }

    #[test]
    fn test_number_format_converter_error() {
        let converter = NumberFormatConverter::new();
        assert!(converter.convert("not a number").is_err());
    }

    #[test]
    fn test_conversion_error_display() {
        let err = ConversionError {
            message: "test error".to_string(),
        };
        assert!(err.to_string().contains("test error"));
    }

    // =========================================================================
    // Widget/State Update Tests
    // =========================================================================

    #[test]
    fn test_widget_update_struct() {
        let update = WidgetUpdate {
            widget_id: "input1".to_string(),
            property: "value".to_string(),
            value: "Hello".to_string(),
        };
        assert_eq!(update.widget_id, "input1");
    }

    #[test]
    fn test_state_update_struct() {
        let update = StateUpdate {
            path: PropertyPath::new("user.name"),
            value: "Alice".to_string(),
        };
        assert_eq!(update.path.to_string_path(), "user.name");
    }

    #[test]
    fn test_binding_id_default() {
        let id = BindingId::default();
        assert_eq!(id.0, 0);
    }

    #[test]
    fn test_update_source_eq() {
        assert_eq!(UpdateSource::State, UpdateSource::State);
        assert_eq!(UpdateSource::Widget, UpdateSource::Widget);
        assert_ne!(UpdateSource::State, UpdateSource::Widget);
    }
}
