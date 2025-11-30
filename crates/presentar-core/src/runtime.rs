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
        self.data
            .lock()
            .expect("MemoryStorage mutex poisoned")
            .len()
    }

    /// Check if storage is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.data
            .lock()
            .expect("MemoryStorage mutex poisoned")
            .is_empty()
    }

    /// Clear all stored data.
    pub fn clear(&self) {
        self.data
            .lock()
            .expect("MemoryStorage mutex poisoned")
            .clear();
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
        self.data
            .lock()
            .expect("MemoryStorage mutex poisoned")
            .get(key)
            .cloned()
    }

    fn remove(&self, key: &str) {
        self.data
            .lock()
            .expect("MemoryStorage mutex poisoned")
            .remove(key);
    }

    fn contains(&self, key: &str) -> bool {
        self.data
            .lock()
            .expect("MemoryStorage mutex poisoned")
            .contains_key(key)
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
        self.history
            .lock()
            .expect("MemoryRouter mutex poisoned")
            .clone()
    }

    /// Get history length.
    #[must_use]
    pub fn history_len(&self) -> usize {
        self.history
            .lock()
            .expect("MemoryRouter mutex poisoned")
            .len()
    }
}

impl Router for MemoryRouter {
    fn navigate(&self, route: &str) {
        let mut current = self.route.lock().expect("MemoryRouter mutex poisoned");
        *current = route.to_string();
        self.history
            .lock()
            .expect("MemoryRouter mutex poisoned")
            .push(route.to_string());
    }

    fn current_route(&self) -> String {
        self.route
            .lock()
            .expect("MemoryRouter mutex poisoned")
            .clone()
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
    pub const fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }

    /// Check if there are messages.
    #[must_use]
    pub const fn has_messages(&self) -> bool {
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
    pub const fn new(config: ExecutorConfig<R, S>) -> Self {
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

// =============================================================================
// Focus Management
// =============================================================================

/// Focus direction for keyboard navigation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusDirection {
    /// Move focus forward (Tab)
    Forward,
    /// Move focus backward (Shift+Tab)
    Backward,
    /// Move focus up (Arrow Up)
    Up,
    /// Move focus down (Arrow Down)
    Down,
    /// Move focus left (Arrow Left)
    Left,
    /// Move focus right (Arrow Right)
    Right,
}

/// Manages keyboard focus for widgets.
#[derive(Debug, Default)]
pub struct FocusManager {
    /// Currently focused widget ID
    focused: Option<u64>,
    /// Focus ring (ordered list of focusable widget IDs)
    focus_ring: Vec<u64>,
    /// Focus trap stack (for modals/dialogs)
    traps: Vec<FocusTrap>,
}

/// A focus trap that restricts focus to a subset of widgets.
#[derive(Debug)]
pub struct FocusTrap {
    /// Widget IDs in this trap
    pub widget_ids: Vec<u64>,
    /// Initial focused widget when trap was created
    pub initial_focus: Option<u64>,
}

impl FocusManager {
    /// Create a new focus manager.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the focus ring (ordered list of focusable widgets).
    pub fn set_focus_ring(&mut self, widget_ids: Vec<u64>) {
        self.focus_ring = widget_ids;
    }

    /// Get the currently focused widget ID.
    #[must_use]
    pub const fn focused(&self) -> Option<u64> {
        self.focused
    }

    /// Set focus to a specific widget.
    pub fn focus(&mut self, widget_id: u64) -> bool {
        let available = self.available_focus_ring();
        if available.contains(&widget_id) {
            self.focused = Some(widget_id);
            true
        } else {
            false
        }
    }

    /// Clear focus.
    pub fn blur(&mut self) {
        self.focused = None;
    }

    /// Move focus in a direction.
    pub fn move_focus(&mut self, direction: FocusDirection) -> Option<u64> {
        let ring = self.available_focus_ring();
        if ring.is_empty() {
            return None;
        }

        let current_idx = self
            .focused
            .and_then(|f| ring.iter().position(|&id| id == f));

        let next_idx = match direction {
            FocusDirection::Forward | FocusDirection::Down | FocusDirection::Right => {
                match current_idx {
                    Some(idx) => (idx + 1) % ring.len(),
                    None => 0,
                }
            }
            FocusDirection::Backward | FocusDirection::Up | FocusDirection::Left => {
                match current_idx {
                    Some(0) | None => ring.len() - 1,
                    Some(idx) => idx - 1,
                }
            }
        };

        let next_id = ring[next_idx];
        self.focused = Some(next_id);
        Some(next_id)
    }

    /// Push a focus trap (for modals/dialogs).
    pub fn push_trap(&mut self, widget_ids: Vec<u64>) {
        let initial = self.focused;
        self.traps.push(FocusTrap {
            widget_ids,
            initial_focus: initial,
        });
        // Focus first item in trap
        if let Some(first) = self.available_focus_ring().first().copied() {
            self.focused = Some(first);
        }
    }

    /// Pop the current focus trap.
    pub fn pop_trap(&mut self) -> Option<FocusTrap> {
        let trap = self.traps.pop();
        // Restore previous focus
        if let Some(ref t) = trap {
            self.focused = t.initial_focus;
        }
        trap
    }

    /// Check if focus is currently trapped.
    #[must_use]
    pub fn is_trapped(&self) -> bool {
        !self.traps.is_empty()
    }

    /// Get the available focus ring (respecting traps).
    fn available_focus_ring(&self) -> Vec<u64> {
        if let Some(trap) = self.traps.last() {
            trap.widget_ids.clone()
        } else {
            self.focus_ring.clone()
        }
    }

    /// Check if a widget is focusable.
    #[must_use]
    pub fn is_focusable(&self, widget_id: u64) -> bool {
        self.available_focus_ring().contains(&widget_id)
    }
}

// =============================================================================
// Animation & Timer System
// =============================================================================

/// Easing functions for smooth animations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EasingFunction {
    /// Linear interpolation (no easing)
    #[default]
    Linear,
    /// Quadratic ease in
    EaseInQuad,
    /// Quadratic ease out
    EaseOutQuad,
    /// Quadratic ease in-out
    EaseInOutQuad,
    /// Cubic ease in
    EaseInCubic,
    /// Cubic ease out
    EaseOutCubic,
    /// Cubic ease in-out
    EaseInOutCubic,
    /// Elastic ease out (spring-like)
    EaseOutElastic,
    /// Bounce ease out
    EaseOutBounce,
}

impl EasingFunction {
    /// Apply the easing function to a normalized time value (0.0 to 1.0).
    #[must_use]
    #[allow(clippy::suboptimal_flops)]
    pub fn apply(self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match self {
            Self::Linear => t,
            Self::EaseInQuad => t * t,
            Self::EaseOutQuad => 1.0 - (1.0 - t) * (1.0 - t),
            Self::EaseInOutQuad => {
                if t < 0.5 {
                    2.0 * t * t
                } else {
                    1.0 - (-2.0 * t + 2.0).powi(2) / 2.0
                }
            }
            Self::EaseInCubic => t * t * t,
            Self::EaseOutCubic => 1.0 - (1.0 - t).powi(3),
            Self::EaseInOutCubic => {
                if t < 0.5 {
                    4.0 * t * t * t
                } else {
                    1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
                }
            }
            Self::EaseOutElastic => {
                if t == 0.0 || t == 1.0 {
                    t
                } else {
                    let c4 = (2.0 * std::f32::consts::PI) / 3.0;
                    2.0_f32.powf(-10.0 * t) * ((t * 10.0 - 0.75) * c4).sin() + 1.0
                }
            }
            Self::EaseOutBounce => {
                let n1 = 7.5625;
                let d1 = 2.75;
                if t < 1.0 / d1 {
                    n1 * t * t
                } else if t < 2.0 / d1 {
                    let t = t - 1.5 / d1;
                    n1 * t * t + 0.75
                } else if t < 2.5 / d1 {
                    let t = t - 2.25 / d1;
                    n1 * t * t + 0.9375
                } else {
                    let t = t - 2.625 / d1;
                    n1 * t * t + 0.984_375
                }
            }
        }
    }
}

/// A tween that interpolates between two values over time.
#[derive(Debug, Clone)]
pub struct Tween<T> {
    /// Starting value
    pub from: T,
    /// Ending value
    pub to: T,
    /// Duration in milliseconds
    pub duration_ms: u32,
    /// Easing function
    pub easing: EasingFunction,
    /// Current elapsed time in milliseconds
    elapsed_ms: u32,
}

impl<T: Clone> Tween<T> {
    /// Create a new tween.
    pub fn new(from: T, to: T, duration_ms: u32) -> Self {
        Self {
            from,
            to,
            duration_ms,
            easing: EasingFunction::default(),
            elapsed_ms: 0,
        }
    }

    /// Set the easing function.
    #[must_use]
    pub const fn with_easing(mut self, easing: EasingFunction) -> Self {
        self.easing = easing;
        self
    }

    /// Get the normalized progress (0.0 to 1.0).
    #[must_use]
    pub fn progress(&self) -> f32 {
        if self.duration_ms == 0 {
            1.0
        } else {
            (self.elapsed_ms as f32 / self.duration_ms as f32).min(1.0)
        }
    }

    /// Get the eased progress value.
    #[must_use]
    pub fn eased_progress(&self) -> f32 {
        self.easing.apply(self.progress())
    }

    /// Check if the tween is complete.
    #[must_use]
    pub const fn is_complete(&self) -> bool {
        self.elapsed_ms >= self.duration_ms
    }

    /// Advance the tween by delta milliseconds.
    pub fn advance(&mut self, delta_ms: u32) {
        self.elapsed_ms = self
            .elapsed_ms
            .saturating_add(delta_ms)
            .min(self.duration_ms);
    }

    /// Reset the tween to the beginning.
    pub fn reset(&mut self) {
        self.elapsed_ms = 0;
    }
}

impl Tween<f32> {
    /// Get the current interpolated value.
    #[must_use]
    #[allow(clippy::suboptimal_flops)]
    pub fn value(&self) -> f32 {
        let t = self.eased_progress();
        self.from + (self.to - self.from) * t
    }
}

impl Tween<f64> {
    /// Get the current interpolated value.
    #[must_use]
    #[allow(clippy::suboptimal_flops)]
    pub fn value(&self) -> f64 {
        let t = f64::from(self.eased_progress());
        self.from + (self.to - self.from) * t
    }
}

/// Animation state for tracking animation lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimationState {
    /// Animation is idle/not started
    Idle,
    /// Animation is running
    Running,
    /// Animation is paused
    Paused,
    /// Animation has completed
    Completed,
}

/// Unique identifier for an animation.
pub type AnimationId = u64;

/// An animation instance that can be managed by an Animator.
#[derive(Debug)]
pub struct AnimationInstance {
    /// Unique ID
    pub id: AnimationId,
    /// Tween for the animation
    pub tween: Tween<f32>,
    /// Current state
    pub state: AnimationState,
    /// Loop count (0 = infinite, 1 = once, N = N times)
    pub loop_count: u32,
    /// Current loop iteration
    pub current_loop: u32,
    /// Whether to reverse on alternate loops (ping-pong)
    pub alternate: bool,
    /// Direction (true = forward, false = reverse)
    forward: bool,
}

impl AnimationInstance {
    /// Create a new animation instance.
    pub fn new(id: AnimationId, from: f32, to: f32, duration_ms: u32) -> Self {
        Self {
            id,
            tween: Tween::new(from, to, duration_ms),
            state: AnimationState::Idle,
            loop_count: 1,
            current_loop: 0,
            alternate: false,
            forward: true,
        }
    }

    /// Set easing function.
    #[must_use]
    pub const fn with_easing(mut self, easing: EasingFunction) -> Self {
        self.tween = self.tween.with_easing(easing);
        self
    }

    /// Set loop count (0 = infinite).
    #[must_use]
    pub const fn with_loop_count(mut self, count: u32) -> Self {
        self.loop_count = count;
        self
    }

    /// Enable ping-pong alternating.
    #[must_use]
    pub const fn with_alternate(mut self, alternate: bool) -> Self {
        self.alternate = alternate;
        self
    }

    /// Start the animation.
    pub fn start(&mut self) {
        self.state = AnimationState::Running;
        self.current_loop = 0;
        self.forward = true;
        self.tween.reset();
    }

    /// Pause the animation.
    pub fn pause(&mut self) {
        if self.state == AnimationState::Running {
            self.state = AnimationState::Paused;
        }
    }

    /// Resume the animation.
    pub fn resume(&mut self) {
        if self.state == AnimationState::Paused {
            self.state = AnimationState::Running;
        }
    }

    /// Stop the animation.
    pub fn stop(&mut self) {
        self.state = AnimationState::Idle;
        self.tween.reset();
    }

    /// Get the current value.
    #[must_use]
    #[allow(clippy::suboptimal_flops)]
    pub fn value(&self) -> f32 {
        if self.forward {
            self.tween.value()
        } else {
            self.tween.from
                + (self.tween.to - self.tween.from) * (1.0 - self.tween.eased_progress())
        }
    }

    /// Advance the animation by delta milliseconds.
    pub fn advance(&mut self, delta_ms: u32) {
        if self.state != AnimationState::Running {
            return;
        }

        self.tween.advance(delta_ms);

        if self.tween.is_complete() {
            // Handle looping
            if self.loop_count == 0 || self.current_loop + 1 < self.loop_count {
                self.current_loop += 1;
                self.tween.reset();

                if self.alternate {
                    self.forward = !self.forward;
                }
            } else {
                self.state = AnimationState::Completed;
            }
        }
    }
}

/// Manages multiple animations.
#[derive(Debug, Default)]
pub struct Animator {
    animations: Vec<AnimationInstance>,
    next_id: AnimationId,
}

impl Animator {
    /// Create a new animator.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a new animation and return its ID.
    pub fn create(&mut self, from: f32, to: f32, duration_ms: u32) -> AnimationId {
        let id = self.next_id;
        self.next_id += 1;
        self.animations
            .push(AnimationInstance::new(id, from, to, duration_ms));
        id
    }

    /// Get an animation by ID.
    #[must_use]
    pub fn get(&self, id: AnimationId) -> Option<&AnimationInstance> {
        self.animations.iter().find(|a| a.id == id)
    }

    /// Get a mutable animation by ID.
    pub fn get_mut(&mut self, id: AnimationId) -> Option<&mut AnimationInstance> {
        self.animations.iter_mut().find(|a| a.id == id)
    }

    /// Start an animation.
    pub fn start(&mut self, id: AnimationId) {
        if let Some(anim) = self.get_mut(id) {
            anim.start();
        }
    }

    /// Pause an animation.
    pub fn pause(&mut self, id: AnimationId) {
        if let Some(anim) = self.get_mut(id) {
            anim.pause();
        }
    }

    /// Resume an animation.
    pub fn resume(&mut self, id: AnimationId) {
        if let Some(anim) = self.get_mut(id) {
            anim.resume();
        }
    }

    /// Stop an animation.
    pub fn stop(&mut self, id: AnimationId) {
        if let Some(anim) = self.get_mut(id) {
            anim.stop();
        }
    }

    /// Remove an animation.
    pub fn remove(&mut self, id: AnimationId) {
        self.animations.retain(|a| a.id != id);
    }

    /// Advance all animations by delta milliseconds.
    pub fn advance(&mut self, delta_ms: u32) {
        for anim in &mut self.animations {
            anim.advance(delta_ms);
        }
    }

    /// Get the value of an animation.
    #[must_use]
    pub fn value(&self, id: AnimationId) -> Option<f32> {
        self.get(id).map(AnimationInstance::value)
    }

    /// Get the number of animations.
    #[must_use]
    pub fn len(&self) -> usize {
        self.animations.len()
    }

    /// Check if there are no animations.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.animations.is_empty()
    }

    /// Remove all completed animations.
    pub fn cleanup_completed(&mut self) {
        self.animations
            .retain(|a| a.state != AnimationState::Completed);
    }

    /// Check if any animations are running.
    #[must_use]
    pub fn has_running(&self) -> bool {
        self.animations
            .iter()
            .any(|a| a.state == AnimationState::Running)
    }
}

/// A timer that fires at regular intervals.
#[derive(Debug)]
pub struct Timer {
    /// Interval in milliseconds
    pub interval_ms: u32,
    /// Elapsed time since last tick
    elapsed_ms: u32,
    /// Whether the timer is running
    running: bool,
    /// Number of times the timer has fired
    tick_count: u64,
    /// Optional limit on tick count (0 = unlimited)
    max_ticks: u64,
}

impl Timer {
    /// Create a new timer with the given interval.
    #[must_use]
    pub const fn new(interval_ms: u32) -> Self {
        Self {
            interval_ms,
            elapsed_ms: 0,
            running: false,
            tick_count: 0,
            max_ticks: 0,
        }
    }

    /// Set maximum tick count (0 = unlimited).
    #[must_use]
    pub const fn with_max_ticks(mut self, max: u64) -> Self {
        self.max_ticks = max;
        self
    }

    /// Start the timer.
    pub fn start(&mut self) {
        self.running = true;
    }

    /// Stop the timer.
    pub fn stop(&mut self) {
        self.running = false;
    }

    /// Reset the timer.
    pub fn reset(&mut self) {
        self.elapsed_ms = 0;
        self.tick_count = 0;
    }

    /// Check if the timer is running.
    #[must_use]
    pub const fn is_running(&self) -> bool {
        self.running
    }

    /// Get the tick count.
    #[must_use]
    pub const fn tick_count(&self) -> u64 {
        self.tick_count
    }

    /// Advance the timer and return the number of ticks that occurred.
    pub fn advance(&mut self, delta_ms: u32) -> u32 {
        if !self.running || self.interval_ms == 0 {
            return 0;
        }

        self.elapsed_ms += delta_ms;
        let ticks = self.elapsed_ms / self.interval_ms;
        self.elapsed_ms %= self.interval_ms;

        // Apply ticks with limit check
        let mut actual_ticks = 0;
        for _ in 0..ticks {
            if self.max_ticks > 0 && self.tick_count >= self.max_ticks {
                self.running = false;
                break;
            }
            self.tick_count += 1;
            actual_ticks += 1;
        }

        actual_ticks
    }

    /// Get progress to next tick (0.0 to 1.0).
    #[must_use]
    pub fn progress(&self) -> f32 {
        if self.interval_ms == 0 {
            0.0
        } else {
            self.elapsed_ms as f32 / self.interval_ms as f32
        }
    }
}

/// Frame timer for 60fps animations.
#[derive(Debug)]
pub struct FrameTimer {
    /// Target frame duration in microseconds (16667 for 60fps)
    target_frame_us: u64,
    /// Last frame timestamp in microseconds
    last_frame_us: Option<u64>,
    /// Accumulated frame time for averaging
    frame_times: [u64; 60],
    /// Current frame index
    frame_index: usize,
    /// Number of recorded frame deltas
    delta_count: usize,
    /// Total frames rendered
    total_frames: u64,
}

impl Default for FrameTimer {
    fn default() -> Self {
        Self::new(60)
    }
}

impl FrameTimer {
    /// Create a new frame timer with target FPS.
    #[must_use]
    pub fn new(target_fps: u32) -> Self {
        let target_frame_us = if target_fps > 0 {
            1_000_000 / u64::from(target_fps)
        } else {
            16667
        };
        Self {
            target_frame_us,
            last_frame_us: None,
            frame_times: [0; 60],
            frame_index: 0,
            delta_count: 0,
            total_frames: 0,
        }
    }

    /// Record a frame with the current timestamp in microseconds.
    pub fn frame(&mut self, now_us: u64) {
        if let Some(last) = self.last_frame_us {
            let delta = now_us.saturating_sub(last);
            self.frame_times[self.frame_index] = delta;
            self.frame_index = (self.frame_index + 1) % 60;
            self.delta_count = (self.delta_count + 1).min(60);
        }
        self.last_frame_us = Some(now_us);
        self.total_frames += 1;
    }

    /// Get the average frame time in microseconds.
    #[must_use]
    pub fn average_frame_time_us(&self) -> u64 {
        if self.delta_count == 0 {
            return self.target_frame_us;
        }
        let sum: u64 = self.frame_times[..self.delta_count].iter().sum();
        sum / self.delta_count as u64
    }

    /// Get the current FPS.
    #[must_use]
    pub fn fps(&self) -> f32 {
        let avg = self.average_frame_time_us();
        if avg == 0 {
            0.0
        } else {
            1_000_000.0 / avg as f32
        }
    }

    /// Check if we're hitting target FPS (within 10% tolerance).
    #[must_use]
    pub fn is_on_target(&self) -> bool {
        let avg = self.average_frame_time_us();
        let target = self.target_frame_us;
        // Within 10% of target
        avg <= target + target / 10
    }

    /// Get the target frame time in milliseconds.
    #[must_use]
    pub fn target_frame_ms(&self) -> f32 {
        self.target_frame_us as f32 / 1000.0
    }

    /// Get total frames rendered.
    #[must_use]
    pub const fn total_frames(&self) -> u64 {
        self.total_frames
    }
}

// =============================================================================
// Data Refresh Manager
// =============================================================================

/// Manages periodic data refresh for data sources.
#[derive(Debug)]
pub struct DataRefreshManager {
    /// Registered refresh tasks
    tasks: Vec<RefreshTask>,
    /// Current timestamp in milliseconds
    current_time_ms: u64,
}

/// A scheduled data refresh task.
#[derive(Debug, Clone)]
pub struct RefreshTask {
    /// Data source key
    pub key: String,
    /// Refresh interval in milliseconds
    pub interval_ms: u64,
    /// Last refresh timestamp
    pub last_refresh_ms: u64,
    /// Whether task is active
    pub active: bool,
}

impl DataRefreshManager {
    /// Create a new refresh manager.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            tasks: Vec::new(),
            current_time_ms: 0,
        }
    }

    /// Register a data source for periodic refresh.
    ///
    /// # Arguments
    ///
    /// * `key` - Data source identifier
    /// * `interval_ms` - Refresh interval in milliseconds
    pub fn register(&mut self, key: impl Into<String>, interval_ms: u64) {
        let key = key.into();

        // Check if already registered
        if let Some(task) = self.tasks.iter_mut().find(|t| t.key == key) {
            task.interval_ms = interval_ms;
            task.active = true;
            return;
        }

        self.tasks.push(RefreshTask {
            key,
            interval_ms,
            last_refresh_ms: 0,
            active: true,
        });
    }

    /// Unregister a data source.
    pub fn unregister(&mut self, key: &str) {
        self.tasks.retain(|t| t.key != key);
    }

    /// Pause refresh for a data source.
    pub fn pause(&mut self, key: &str) {
        if let Some(task) = self.tasks.iter_mut().find(|t| t.key == key) {
            task.active = false;
        }
    }

    /// Resume refresh for a data source.
    pub fn resume(&mut self, key: &str) {
        if let Some(task) = self.tasks.iter_mut().find(|t| t.key == key) {
            task.active = true;
        }
    }

    /// Update the manager with the current timestamp.
    ///
    /// Returns keys of data sources that need to be refreshed.
    pub fn update(&mut self, current_time_ms: u64) -> Vec<String> {
        self.current_time_ms = current_time_ms;

        let mut to_refresh = Vec::new();

        for task in &mut self.tasks {
            if !task.active {
                continue;
            }

            let elapsed = current_time_ms.saturating_sub(task.last_refresh_ms);
            if elapsed >= task.interval_ms {
                to_refresh.push(task.key.clone());
                task.last_refresh_ms = current_time_ms;
            }
        }

        to_refresh
    }

    /// Force immediate refresh of a data source.
    pub fn force_refresh(&mut self, key: &str) -> bool {
        if let Some(task) = self.tasks.iter_mut().find(|t| t.key == key) {
            task.last_refresh_ms = 0;
            true
        } else {
            false
        }
    }

    /// Get all registered tasks.
    #[must_use]
    pub fn tasks(&self) -> &[RefreshTask] {
        &self.tasks
    }

    /// Get task by key.
    #[must_use]
    pub fn get_task(&self, key: &str) -> Option<&RefreshTask> {
        self.tasks.iter().find(|t| t.key == key)
    }

    /// Check if a data source is due for refresh.
    #[must_use]
    pub fn is_due(&self, key: &str) -> bool {
        if let Some(task) = self.tasks.iter().find(|t| t.key == key) {
            if !task.active {
                return false;
            }
            let elapsed = self.current_time_ms.saturating_sub(task.last_refresh_ms);
            elapsed >= task.interval_ms
        } else {
            false
        }
    }

    /// Get time until next refresh for a data source (in ms).
    #[must_use]
    pub fn time_until_refresh(&self, key: &str) -> Option<u64> {
        self.tasks.iter().find(|t| t.key == key).map(|task| {
            if !task.active {
                return u64::MAX;
            }
            let elapsed = self.current_time_ms.saturating_sub(task.last_refresh_ms);
            task.interval_ms.saturating_sub(elapsed)
        })
    }
}

impl Default for DataRefreshManager {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// Widget Animation API
// =============================================================================

/// Configuration for property transitions.
#[derive(Debug, Clone)]
pub struct TransitionConfig {
    /// Duration in milliseconds
    pub duration_ms: u32,
    /// Easing function
    pub easing: EasingFunction,
    /// Delay before starting in milliseconds
    pub delay_ms: u32,
}

impl Default for TransitionConfig {
    fn default() -> Self {
        Self {
            duration_ms: 300,
            easing: EasingFunction::EaseInOutCubic,
            delay_ms: 0,
        }
    }
}

impl TransitionConfig {
    /// Create a new transition configuration.
    #[must_use]
    pub const fn new(duration_ms: u32) -> Self {
        Self {
            duration_ms,
            easing: EasingFunction::EaseInOutCubic,
            delay_ms: 0,
        }
    }

    /// Set the easing function.
    #[must_use]
    pub const fn with_easing(mut self, easing: EasingFunction) -> Self {
        self.easing = easing;
        self
    }

    /// Set the delay.
    #[must_use]
    pub const fn with_delay(mut self, delay_ms: u32) -> Self {
        self.delay_ms = delay_ms;
        self
    }

    /// Quick preset (150ms)
    #[must_use]
    pub const fn quick() -> Self {
        Self::new(150)
    }

    /// Normal preset (300ms)
    #[must_use]
    pub const fn normal() -> Self {
        Self::new(300)
    }

    /// Slow preset (500ms)
    #[must_use]
    pub const fn slow() -> Self {
        Self::new(500)
    }
}

/// An animated property that smoothly transitions between values.
///
/// Use this in widget state to animate property changes automatically.
#[derive(Debug, Clone)]
pub struct AnimatedProperty<T> {
    /// Current visual value (what's rendered)
    current: T,
    /// Target value we're animating towards
    target: T,
    /// Starting value of current animation
    start: T,
    /// Transition configuration
    config: TransitionConfig,
    /// Elapsed time in milliseconds
    elapsed_ms: u32,
    /// Whether an animation is in progress
    animating: bool,
}

impl<T: Clone + Default> Default for AnimatedProperty<T> {
    fn default() -> Self {
        Self::new(T::default())
    }
}

impl<T: Clone> AnimatedProperty<T> {
    /// Create a new animated property with an initial value.
    pub fn new(value: T) -> Self {
        Self {
            current: value.clone(),
            target: value.clone(),
            start: value,
            config: TransitionConfig::default(),
            elapsed_ms: 0,
            animating: false,
        }
    }

    /// Create with a custom transition config.
    pub fn with_config(value: T, config: TransitionConfig) -> Self {
        Self {
            current: value.clone(),
            target: value.clone(),
            start: value,
            config,
            elapsed_ms: 0,
            animating: false,
        }
    }

    /// Get the current visual value.
    pub const fn get(&self) -> &T {
        &self.current
    }

    /// Get the target value.
    pub const fn target(&self) -> &T {
        &self.target
    }

    /// Check if currently animating.
    #[must_use]
    pub const fn is_animating(&self) -> bool {
        self.animating
    }

    /// Set a new target value, starting an animation.
    pub fn set(&mut self, value: T) {
        self.start = self.current.clone();
        self.target = value;
        self.elapsed_ms = 0;
        self.animating = true;
    }

    /// Set value immediately without animation.
    pub fn set_immediate(&mut self, value: T) {
        self.current = value.clone();
        self.target = value.clone();
        self.start = value;
        self.animating = false;
        self.elapsed_ms = 0;
    }

    /// Get animation progress (0.0 to 1.0).
    #[must_use]
    pub fn progress(&self) -> f32 {
        if !self.animating {
            return 1.0;
        }

        let total = self.config.duration_ms + self.config.delay_ms;
        if total == 0 {
            return 1.0;
        }

        if self.elapsed_ms < self.config.delay_ms {
            return 0.0;
        }

        let elapsed_after_delay = self.elapsed_ms - self.config.delay_ms;
        (elapsed_after_delay as f32 / self.config.duration_ms as f32).min(1.0)
    }

    /// Get eased progress.
    #[must_use]
    pub fn eased_progress(&self) -> f32 {
        self.config.easing.apply(self.progress())
    }
}

impl AnimatedProperty<f32> {
    /// Advance the animation by delta milliseconds.
    pub fn advance(&mut self, delta_ms: u32) {
        if !self.animating {
            return;
        }

        self.elapsed_ms += delta_ms;

        let t = self.eased_progress();
        self.current = (self.target - self.start).mul_add(t, self.start);

        if self.progress() >= 1.0 {
            self.current = self.target;
            self.animating = false;
        }
    }
}

impl AnimatedProperty<f64> {
    /// Advance the animation by delta milliseconds.
    pub fn advance(&mut self, delta_ms: u32) {
        if !self.animating {
            return;
        }

        self.elapsed_ms += delta_ms;

        let t = f64::from(self.eased_progress());
        self.current = (self.target - self.start).mul_add(t, self.start);

        if self.progress() >= 1.0 {
            self.current = self.target;
            self.animating = false;
        }
    }
}

impl AnimatedProperty<crate::Color> {
    /// Advance the animation by delta milliseconds.
    pub fn advance(&mut self, delta_ms: u32) {
        if !self.animating {
            return;
        }

        self.elapsed_ms += delta_ms;

        let t = self.eased_progress();
        self.current = crate::Color {
            r: (self.target.r - self.start.r).mul_add(t, self.start.r),
            g: (self.target.g - self.start.g).mul_add(t, self.start.g),
            b: (self.target.b - self.start.b).mul_add(t, self.start.b),
            a: (self.target.a - self.start.a).mul_add(t, self.start.a),
        };

        if self.progress() >= 1.0 {
            self.current = self.target;
            self.animating = false;
        }
    }
}

impl AnimatedProperty<crate::Point> {
    /// Advance the animation by delta milliseconds.
    pub fn advance(&mut self, delta_ms: u32) {
        if !self.animating {
            return;
        }

        self.elapsed_ms += delta_ms;

        let t = self.eased_progress();
        self.current = crate::Point {
            x: (self.target.x - self.start.x).mul_add(t, self.start.x),
            y: (self.target.y - self.start.y).mul_add(t, self.start.y),
        };

        if self.progress() >= 1.0 {
            self.current = self.target;
            self.animating = false;
        }
    }
}

impl AnimatedProperty<crate::Size> {
    /// Advance the animation by delta milliseconds.
    pub fn advance(&mut self, delta_ms: u32) {
        if !self.animating {
            return;
        }

        self.elapsed_ms += delta_ms;

        let t = self.eased_progress();
        self.current = crate::Size {
            width: (self.target.width - self.start.width).mul_add(t, self.start.width),
            height: (self.target.height - self.start.height).mul_add(t, self.start.height),
        };

        if self.progress() >= 1.0 {
            self.current = self.target;
            self.animating = false;
        }
    }
}

/// Spring animation configuration.
#[derive(Debug, Clone, Copy)]
pub struct SpringConfig {
    /// Spring stiffness (higher = faster oscillation)
    pub stiffness: f32,
    /// Damping (higher = less oscillation)
    pub damping: f32,
    /// Mass of the object
    pub mass: f32,
}

impl Default for SpringConfig {
    fn default() -> Self {
        Self {
            stiffness: 100.0,
            damping: 10.0,
            mass: 1.0,
        }
    }
}

impl SpringConfig {
    /// Create a new spring configuration.
    #[must_use]
    pub const fn new(stiffness: f32, damping: f32, mass: f32) -> Self {
        Self {
            stiffness,
            damping,
            mass,
        }
    }

    /// Gentle spring preset.
    #[must_use]
    pub const fn gentle() -> Self {
        Self::new(100.0, 15.0, 1.0)
    }

    /// Bouncy spring preset.
    #[must_use]
    pub const fn bouncy() -> Self {
        Self::new(300.0, 10.0, 1.0)
    }

    /// Stiff spring preset.
    #[must_use]
    pub const fn stiff() -> Self {
        Self::new(500.0, 30.0, 1.0)
    }
}

/// Spring-based animation for physics-like motion.
#[derive(Debug, Clone)]
pub struct SpringAnimation {
    /// Current position
    position: f32,
    /// Current velocity
    velocity: f32,
    /// Target position
    target: f32,
    /// Spring configuration
    config: SpringConfig,
    /// Velocity threshold for considering animation complete
    velocity_threshold: f32,
    /// Position threshold for considering animation complete
    position_threshold: f32,
}

impl SpringAnimation {
    /// Create a new spring animation.
    #[must_use]
    pub fn new(initial: f32) -> Self {
        Self {
            position: initial,
            velocity: 0.0,
            target: initial,
            config: SpringConfig::default(),
            velocity_threshold: 0.01,
            position_threshold: 0.001,
        }
    }

    /// Create with custom spring config.
    #[must_use]
    pub const fn with_config(initial: f32, config: SpringConfig) -> Self {
        Self {
            position: initial,
            velocity: 0.0,
            target: initial,
            config,
            velocity_threshold: 0.01,
            position_threshold: 0.001,
        }
    }

    /// Get the current position.
    #[must_use]
    pub const fn position(&self) -> f32 {
        self.position
    }

    /// Get the current velocity.
    #[must_use]
    pub const fn velocity(&self) -> f32 {
        self.velocity
    }

    /// Get the target.
    #[must_use]
    pub const fn target(&self) -> f32 {
        self.target
    }

    /// Set the target position.
    pub fn set_target(&mut self, target: f32) {
        self.target = target;
    }

    /// Set position immediately without animation.
    pub fn set_immediate(&mut self, position: f32) {
        self.position = position;
        self.target = position;
        self.velocity = 0.0;
    }

    /// Check if the animation is at rest.
    #[must_use]
    pub fn is_at_rest(&self) -> bool {
        let position_diff = (self.position - self.target).abs();
        let velocity_abs = self.velocity.abs();
        position_diff < self.position_threshold && velocity_abs < self.velocity_threshold
    }

    /// Advance the spring animation by delta seconds.
    pub fn advance(&mut self, delta_s: f32) {
        if self.is_at_rest() {
            self.position = self.target;
            self.velocity = 0.0;
            return;
        }

        // Spring physics: F = -kx - cv
        // a = F/m = (-kx - cv) / m
        let displacement = self.position - self.target;
        let spring_force = -self.config.stiffness * displacement;
        let damping_force = -self.config.damping * self.velocity;
        let acceleration = (spring_force + damping_force) / self.config.mass;

        // Semi-implicit Euler integration
        self.velocity += acceleration * delta_s;
        self.position += self.velocity * delta_s;
    }

    /// Advance by delta milliseconds.
    pub fn advance_ms(&mut self, delta_ms: u32) {
        self.advance(delta_ms as f32 / 1000.0);
    }
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

    // =========================================================================
    // FocusManager Tests
    // =========================================================================

    #[test]
    fn test_focus_manager_new() {
        let fm = FocusManager::new();
        assert!(fm.focused().is_none());
        assert!(!fm.is_trapped());
    }

    #[test]
    fn test_focus_manager_set_ring() {
        let mut fm = FocusManager::new();
        fm.set_focus_ring(vec![1, 2, 3]);
        assert!(fm.is_focusable(1));
        assert!(fm.is_focusable(2));
        assert!(!fm.is_focusable(4));
    }

    #[test]
    fn test_focus_manager_focus() {
        let mut fm = FocusManager::new();
        fm.set_focus_ring(vec![1, 2, 3]);

        assert!(fm.focus(2));
        assert_eq!(fm.focused(), Some(2));

        // Can't focus non-focusable widget
        assert!(!fm.focus(99));
        assert_eq!(fm.focused(), Some(2));
    }

    #[test]
    fn test_focus_manager_blur() {
        let mut fm = FocusManager::new();
        fm.set_focus_ring(vec![1, 2, 3]);
        fm.focus(1);
        assert!(fm.focused().is_some());

        fm.blur();
        assert!(fm.focused().is_none());
    }

    #[test]
    fn test_focus_manager_move_forward() {
        let mut fm = FocusManager::new();
        fm.set_focus_ring(vec![1, 2, 3]);

        // No focus, should focus first
        let next = fm.move_focus(FocusDirection::Forward);
        assert_eq!(next, Some(1));

        // Move forward
        let next = fm.move_focus(FocusDirection::Forward);
        assert_eq!(next, Some(2));

        let next = fm.move_focus(FocusDirection::Forward);
        assert_eq!(next, Some(3));

        // Wrap around
        let next = fm.move_focus(FocusDirection::Forward);
        assert_eq!(next, Some(1));
    }

    #[test]
    fn test_focus_manager_move_backward() {
        let mut fm = FocusManager::new();
        fm.set_focus_ring(vec![1, 2, 3]);

        // No focus, should focus last
        let next = fm.move_focus(FocusDirection::Backward);
        assert_eq!(next, Some(3));

        // Move backward
        let next = fm.move_focus(FocusDirection::Backward);
        assert_eq!(next, Some(2));

        let next = fm.move_focus(FocusDirection::Backward);
        assert_eq!(next, Some(1));

        // Wrap around
        let next = fm.move_focus(FocusDirection::Backward);
        assert_eq!(next, Some(3));
    }

    #[test]
    fn test_focus_manager_empty_ring() {
        let mut fm = FocusManager::new();
        let next = fm.move_focus(FocusDirection::Forward);
        assert!(next.is_none());
    }

    #[test]
    fn test_focus_manager_trap() {
        let mut fm = FocusManager::new();
        fm.set_focus_ring(vec![1, 2, 3, 4, 5]);
        fm.focus(2);

        // Push trap (like opening a modal)
        fm.push_trap(vec![10, 11, 12]);
        assert!(fm.is_trapped());
        assert_eq!(fm.focused(), Some(10)); // Auto-focuses first in trap

        // Can only focus within trap
        assert!(fm.is_focusable(10));
        assert!(!fm.is_focusable(1));

        // Navigate within trap
        fm.move_focus(FocusDirection::Forward);
        assert_eq!(fm.focused(), Some(11));
    }

    #[test]
    fn test_focus_manager_pop_trap() {
        let mut fm = FocusManager::new();
        fm.set_focus_ring(vec![1, 2, 3]);
        fm.focus(2);

        fm.push_trap(vec![10, 11]);
        assert_eq!(fm.focused(), Some(10));

        // Pop trap should restore previous focus
        let trap = fm.pop_trap();
        assert!(trap.is_some());
        assert!(!fm.is_trapped());
        assert_eq!(fm.focused(), Some(2)); // Restored
    }

    #[test]
    fn test_focus_manager_nested_traps() {
        let mut fm = FocusManager::new();
        fm.set_focus_ring(vec![1, 2, 3]);
        fm.focus(1);

        // First trap
        fm.push_trap(vec![10, 11]);
        assert_eq!(fm.focused(), Some(10));

        // Nested trap
        fm.push_trap(vec![20, 21]);
        assert_eq!(fm.focused(), Some(20));

        // Pop inner trap
        fm.pop_trap();
        assert_eq!(fm.focused(), Some(10));

        // Pop outer trap
        fm.pop_trap();
        assert_eq!(fm.focused(), Some(1));
    }

    #[test]
    fn test_focus_direction_variants() {
        let mut fm = FocusManager::new();
        fm.set_focus_ring(vec![1, 2, 3]);
        fm.focus(2);

        // Down/Right act like Forward
        fm.move_focus(FocusDirection::Down);
        assert_eq!(fm.focused(), Some(3));

        fm.focus(2);
        fm.move_focus(FocusDirection::Right);
        assert_eq!(fm.focused(), Some(3));

        // Up/Left act like Backward
        fm.focus(2);
        fm.move_focus(FocusDirection::Up);
        assert_eq!(fm.focused(), Some(1));

        fm.focus(2);
        fm.move_focus(FocusDirection::Left);
        assert_eq!(fm.focused(), Some(1));
    }

    // =========================================================================
    // EasingFunction Tests
    // =========================================================================

    #[test]
    fn test_easing_linear() {
        assert_eq!(EasingFunction::Linear.apply(0.0), 0.0);
        assert_eq!(EasingFunction::Linear.apply(0.5), 0.5);
        assert_eq!(EasingFunction::Linear.apply(1.0), 1.0);
    }

    #[test]
    fn test_easing_clamps_input() {
        assert_eq!(EasingFunction::Linear.apply(-0.5), 0.0);
        assert_eq!(EasingFunction::Linear.apply(1.5), 1.0);
    }

    #[test]
    fn test_easing_quad() {
        // EaseInQuad starts slow
        assert!(EasingFunction::EaseInQuad.apply(0.5) < 0.5);
        // EaseOutQuad ends slow
        assert!(EasingFunction::EaseOutQuad.apply(0.5) > 0.5);
        // Boundaries
        assert_eq!(EasingFunction::EaseInQuad.apply(0.0), 0.0);
        assert_eq!(EasingFunction::EaseInQuad.apply(1.0), 1.0);
    }

    #[test]
    fn test_easing_cubic() {
        assert!(EasingFunction::EaseInCubic.apply(0.5) < 0.5);
        assert!(EasingFunction::EaseOutCubic.apply(0.5) > 0.5);
        assert_eq!(EasingFunction::EaseInCubic.apply(0.0), 0.0);
        assert_eq!(EasingFunction::EaseOutCubic.apply(1.0), 1.0);
    }

    #[test]
    fn test_easing_in_out_quad() {
        // First half accelerates
        let first_quarter = EasingFunction::EaseInOutQuad.apply(0.25);
        assert!(first_quarter < 0.25);
        // Second half decelerates
        let third_quarter = EasingFunction::EaseInOutQuad.apply(0.75);
        assert!(third_quarter > 0.75);
    }

    #[test]
    fn test_easing_elastic() {
        assert_eq!(EasingFunction::EaseOutElastic.apply(0.0), 0.0);
        assert_eq!(EasingFunction::EaseOutElastic.apply(1.0), 1.0);
        // Elastic overshoots then settles
        let mid = EasingFunction::EaseOutElastic.apply(0.5);
        assert!(mid > 0.9); // Already past target due to elastic
    }

    #[test]
    fn test_easing_bounce() {
        assert_eq!(EasingFunction::EaseOutBounce.apply(0.0), 0.0);
        assert!((EasingFunction::EaseOutBounce.apply(1.0) - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_easing_default() {
        assert_eq!(EasingFunction::default(), EasingFunction::Linear);
    }

    // =========================================================================
    // Tween Tests
    // =========================================================================

    #[test]
    fn test_tween_new() {
        let tween = Tween::new(0.0_f32, 100.0, 1000);
        assert_eq!(tween.from, 0.0);
        assert_eq!(tween.to, 100.0);
        assert_eq!(tween.duration_ms, 1000);
        assert_eq!(tween.easing, EasingFunction::Linear);
    }

    #[test]
    fn test_tween_progress() {
        let mut tween = Tween::new(0.0_f32, 100.0, 1000);
        assert_eq!(tween.progress(), 0.0);

        tween.advance(500);
        assert_eq!(tween.progress(), 0.5);

        tween.advance(500);
        assert_eq!(tween.progress(), 1.0);
    }

    #[test]
    fn test_tween_value() {
        let mut tween = Tween::new(0.0_f32, 100.0, 1000);
        assert_eq!(tween.value(), 0.0);

        tween.advance(500);
        assert_eq!(tween.value(), 50.0);

        tween.advance(500);
        assert_eq!(tween.value(), 100.0);
    }

    #[test]
    fn test_tween_f64_value() {
        let mut tween = Tween::new(0.0_f64, 100.0, 1000);
        tween.advance(250);
        assert!((tween.value() - 25.0).abs() < 0.001);
    }

    #[test]
    fn test_tween_with_easing() {
        let mut tween = Tween::new(0.0_f32, 100.0, 1000).with_easing(EasingFunction::EaseInQuad);
        tween.advance(500);
        // With ease-in, value at 50% time should be less than 50
        assert!(tween.value() < 50.0);
    }

    #[test]
    fn test_tween_is_complete() {
        let mut tween = Tween::new(0.0_f32, 100.0, 1000);
        assert!(!tween.is_complete());

        tween.advance(999);
        assert!(!tween.is_complete());

        tween.advance(1);
        assert!(tween.is_complete());
    }

    #[test]
    fn test_tween_reset() {
        let mut tween = Tween::new(0.0_f32, 100.0, 1000);
        tween.advance(500);
        assert_eq!(tween.progress(), 0.5);

        tween.reset();
        assert_eq!(tween.progress(), 0.0);
    }

    #[test]
    fn test_tween_zero_duration() {
        let tween = Tween::new(0.0_f32, 100.0, 0);
        assert_eq!(tween.progress(), 1.0);
        assert!(tween.is_complete());
    }

    #[test]
    fn test_tween_advance_overflow() {
        let mut tween = Tween::new(0.0_f32, 100.0, 1000);
        tween.advance(2000); // Way past duration
        assert_eq!(tween.progress(), 1.0);
        assert!(tween.is_complete());
    }

    // =========================================================================
    // AnimationInstance Tests
    // =========================================================================

    #[test]
    fn test_animation_instance_new() {
        let anim = AnimationInstance::new(1, 0.0, 100.0, 1000);
        assert_eq!(anim.id, 1);
        assert_eq!(anim.state, AnimationState::Idle);
        assert_eq!(anim.loop_count, 1);
    }

    #[test]
    fn test_animation_instance_start() {
        let mut anim = AnimationInstance::new(1, 0.0, 100.0, 1000);
        anim.start();
        assert_eq!(anim.state, AnimationState::Running);
    }

    #[test]
    fn test_animation_instance_pause_resume() {
        let mut anim = AnimationInstance::new(1, 0.0, 100.0, 1000);
        anim.start();
        anim.advance(500);

        anim.pause();
        assert_eq!(anim.state, AnimationState::Paused);

        // Advance while paused does nothing
        anim.advance(500);
        assert!(!anim.tween.is_complete());

        anim.resume();
        assert_eq!(anim.state, AnimationState::Running);
    }

    #[test]
    fn test_animation_instance_stop() {
        let mut anim = AnimationInstance::new(1, 0.0, 100.0, 1000);
        anim.start();
        anim.advance(500);

        anim.stop();
        assert_eq!(anim.state, AnimationState::Idle);
        assert_eq!(anim.tween.progress(), 0.0);
    }

    #[test]
    fn test_animation_instance_complete() {
        let mut anim = AnimationInstance::new(1, 0.0, 100.0, 1000);
        anim.start();
        anim.advance(1000);

        assert_eq!(anim.state, AnimationState::Completed);
    }

    #[test]
    fn test_animation_instance_loop() {
        let mut anim = AnimationInstance::new(1, 0.0, 100.0, 1000).with_loop_count(3);
        anim.start();

        // First loop
        anim.advance(1000);
        assert_eq!(anim.state, AnimationState::Running);
        assert_eq!(anim.current_loop, 1);

        // Second loop
        anim.advance(1000);
        assert_eq!(anim.current_loop, 2);

        // Third loop completes
        anim.advance(1000);
        assert_eq!(anim.state, AnimationState::Completed);
    }

    #[test]
    fn test_animation_instance_infinite_loop() {
        let mut anim = AnimationInstance::new(1, 0.0, 100.0, 1000).with_loop_count(0);
        anim.start();

        for _ in 0..100 {
            anim.advance(1000);
            assert_eq!(anim.state, AnimationState::Running);
        }
    }

    #[test]
    fn test_animation_instance_alternate() {
        let mut anim = AnimationInstance::new(1, 0.0, 100.0, 1000)
            .with_loop_count(2)
            .with_alternate(true);
        anim.start();

        // Forward
        anim.advance(500);
        assert!((anim.value() - 50.0).abs() < 0.001);

        // Complete first loop
        anim.advance(500);
        assert_eq!(anim.current_loop, 1);

        // Now going backward
        anim.advance(500);
        // Value should be going from 100 back toward 0
        assert!((anim.value() - 50.0).abs() < 0.001);
    }

    #[test]
    fn test_animation_instance_with_easing() {
        let anim =
            AnimationInstance::new(1, 0.0, 100.0, 1000).with_easing(EasingFunction::EaseInQuad);
        assert_eq!(anim.tween.easing, EasingFunction::EaseInQuad);
    }

    // =========================================================================
    // Animator Tests
    // =========================================================================

    #[test]
    fn test_animator_new() {
        let animator = Animator::new();
        assert!(animator.is_empty());
        assert_eq!(animator.len(), 0);
    }

    #[test]
    fn test_animator_create() {
        let mut animator = Animator::new();
        let id = animator.create(0.0, 100.0, 1000);

        assert_eq!(animator.len(), 1);
        assert!(animator.get(id).is_some());
    }

    #[test]
    fn test_animator_unique_ids() {
        let mut animator = Animator::new();
        let id1 = animator.create(0.0, 100.0, 1000);
        let id2 = animator.create(0.0, 100.0, 1000);
        let id3 = animator.create(0.0, 100.0, 1000);

        assert_ne!(id1, id2);
        assert_ne!(id2, id3);
    }

    #[test]
    fn test_animator_start_and_value() {
        let mut animator = Animator::new();
        let id = animator.create(0.0, 100.0, 1000);

        animator.start(id);
        assert_eq!(animator.value(id), Some(0.0));

        animator.advance(500);
        assert_eq!(animator.value(id), Some(50.0));
    }

    #[test]
    fn test_animator_pause_resume() {
        let mut animator = Animator::new();
        let id = animator.create(0.0, 100.0, 1000);
        animator.start(id);
        animator.advance(250);

        animator.pause(id);
        animator.advance(500); // Should not advance

        animator.resume(id);
        animator.advance(250);

        // Total should be 500ms (250 + 250, not counting paused time)
        assert_eq!(animator.value(id), Some(50.0));
    }

    #[test]
    fn test_animator_stop() {
        let mut animator = Animator::new();
        let id = animator.create(0.0, 100.0, 1000);
        animator.start(id);
        animator.advance(500);

        animator.stop(id);
        assert_eq!(animator.value(id), Some(0.0));
    }

    #[test]
    fn test_animator_remove() {
        let mut animator = Animator::new();
        let id = animator.create(0.0, 100.0, 1000);
        assert_eq!(animator.len(), 1);

        animator.remove(id);
        assert!(animator.is_empty());
        assert!(animator.get(id).is_none());
    }

    #[test]
    fn test_animator_has_running() {
        let mut animator = Animator::new();
        let id = animator.create(0.0, 100.0, 1000);

        assert!(!animator.has_running());

        animator.start(id);
        assert!(animator.has_running());

        animator.advance(1000);
        assert!(!animator.has_running()); // Completed
    }

    #[test]
    fn test_animator_cleanup_completed() {
        let mut animator = Animator::new();
        let id1 = animator.create(0.0, 100.0, 500);
        let id2 = animator.create(0.0, 100.0, 1000);

        animator.start(id1);
        animator.start(id2);
        animator.advance(500);

        assert_eq!(animator.len(), 2);

        animator.cleanup_completed();
        assert_eq!(animator.len(), 1);
        assert!(animator.get(id1).is_none());
        assert!(animator.get(id2).is_some());
    }

    #[test]
    fn test_animator_multiple_animations() {
        let mut animator = Animator::new();
        let id1 = animator.create(0.0, 100.0, 1000);
        let id2 = animator.create(100.0, 0.0, 1000);

        animator.start(id1);
        animator.start(id2);
        animator.advance(500);

        assert_eq!(animator.value(id1), Some(50.0));
        assert_eq!(animator.value(id2), Some(50.0)); // Going from 100 to 0
    }

    // =========================================================================
    // Timer Tests
    // =========================================================================

    #[test]
    fn test_timer_new() {
        let timer = Timer::new(1000);
        assert_eq!(timer.interval_ms, 1000);
        assert!(!timer.is_running());
        assert_eq!(timer.tick_count(), 0);
    }

    #[test]
    fn test_timer_start_stop() {
        let mut timer = Timer::new(1000);
        timer.start();
        assert!(timer.is_running());

        timer.stop();
        assert!(!timer.is_running());
    }

    #[test]
    fn test_timer_advance() {
        let mut timer = Timer::new(1000);
        timer.start();

        // Advance less than interval
        let ticks = timer.advance(500);
        assert_eq!(ticks, 0);
        assert_eq!(timer.tick_count(), 0);

        // Complete first interval
        let ticks = timer.advance(500);
        assert_eq!(ticks, 1);
        assert_eq!(timer.tick_count(), 1);
    }

    #[test]
    fn test_timer_multiple_ticks() {
        let mut timer = Timer::new(100);
        timer.start();

        let ticks = timer.advance(350);
        assert_eq!(ticks, 3);
        assert_eq!(timer.tick_count(), 3);

        // Remainder should carry over
        assert!((timer.progress() - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_timer_max_ticks() {
        let mut timer = Timer::new(100).with_max_ticks(3);
        timer.start();

        timer.advance(200); // 2 ticks
        assert!(timer.is_running());

        timer.advance(200); // Would be 2 more, but limited to 1
        assert!(!timer.is_running());
        assert_eq!(timer.tick_count(), 3);
    }

    #[test]
    fn test_timer_reset() {
        let mut timer = Timer::new(100);
        timer.start();
        timer.advance(250);
        assert_eq!(timer.tick_count(), 2);

        timer.reset();
        assert_eq!(timer.tick_count(), 0);
        assert_eq!(timer.progress(), 0.0);
    }

    #[test]
    fn test_timer_progress() {
        let mut timer = Timer::new(100);
        timer.start();
        timer.advance(50);
        assert!((timer.progress() - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_timer_zero_interval() {
        let mut timer = Timer::new(0);
        timer.start();
        let ticks = timer.advance(1000);
        assert_eq!(ticks, 0); // Zero interval means no ticks
    }

    #[test]
    fn test_timer_not_running() {
        let mut timer = Timer::new(100);
        let ticks = timer.advance(1000);
        assert_eq!(ticks, 0); // Not started, no ticks
    }

    // =========================================================================
    // FrameTimer Tests
    // =========================================================================

    #[test]
    fn test_frame_timer_new() {
        let ft = FrameTimer::new(60);
        assert_eq!(ft.total_frames(), 0);
        assert!((ft.target_frame_ms() - 16.667).abs() < 0.01);
    }

    #[test]
    fn test_frame_timer_default() {
        let ft = FrameTimer::default();
        assert_eq!(ft.total_frames(), 0);
    }

    #[test]
    fn test_frame_timer_frame() {
        let mut ft = FrameTimer::new(60);
        ft.frame(0);
        assert_eq!(ft.total_frames(), 1);

        ft.frame(16667); // 16.667ms later
        assert_eq!(ft.total_frames(), 2);
    }

    #[test]
    fn test_frame_timer_fps() {
        let mut ft = FrameTimer::new(60);

        // Simulate 60fps
        for i in 0..60 {
            ft.frame(i * 16667);
        }

        let fps = ft.fps();
        assert!(fps > 55.0 && fps < 65.0);
    }

    #[test]
    fn test_frame_timer_is_on_target() {
        let mut ft = FrameTimer::new(60);

        // Perfect 60fps
        for i in 0..10 {
            ft.frame(i * 16667);
        }
        assert!(ft.is_on_target());
    }

    #[test]
    fn test_frame_timer_slow_frames() {
        let mut ft = FrameTimer::new(60);

        // Simulate 30fps (33ms frames)
        for i in 0..10 {
            ft.frame(i * 33333);
        }

        let fps = ft.fps();
        assert!(fps < 35.0);
        assert!(!ft.is_on_target());
    }

    #[test]
    fn test_frame_timer_zero_fps() {
        let ft = FrameTimer::new(0);
        assert!((ft.target_frame_ms() - 16.667).abs() < 0.01); // Falls back to 60fps
    }

    // =========================================================================
    // TransitionConfig Tests
    // =========================================================================

    #[test]
    fn test_transition_config_default() {
        let config = TransitionConfig::default();
        assert_eq!(config.duration_ms, 300);
        assert_eq!(config.delay_ms, 0);
        assert_eq!(config.easing, EasingFunction::EaseInOutCubic);
    }

    #[test]
    fn test_transition_config_new() {
        let config = TransitionConfig::new(500);
        assert_eq!(config.duration_ms, 500);
    }

    #[test]
    fn test_transition_config_presets() {
        assert_eq!(TransitionConfig::quick().duration_ms, 150);
        assert_eq!(TransitionConfig::normal().duration_ms, 300);
        assert_eq!(TransitionConfig::slow().duration_ms, 500);
    }

    #[test]
    fn test_transition_config_builder() {
        let config = TransitionConfig::new(200)
            .with_easing(EasingFunction::EaseOutBounce)
            .with_delay(50);

        assert_eq!(config.duration_ms, 200);
        assert_eq!(config.easing, EasingFunction::EaseOutBounce);
        assert_eq!(config.delay_ms, 50);
    }

    // =========================================================================
    // AnimatedProperty Tests
    // =========================================================================

    #[test]
    fn test_animated_property_new() {
        let prop = AnimatedProperty::new(0.0_f32);
        assert_eq!(*prop.get(), 0.0);
        assert_eq!(*prop.target(), 0.0);
        assert!(!prop.is_animating());
    }

    #[test]
    fn test_animated_property_default() {
        let prop: AnimatedProperty<f32> = AnimatedProperty::default();
        assert_eq!(*prop.get(), 0.0);
    }

    #[test]
    fn test_animated_property_set() {
        let mut prop = AnimatedProperty::new(0.0_f32);
        prop.set(100.0);

        assert!(prop.is_animating());
        assert_eq!(*prop.target(), 100.0);
        assert_eq!(*prop.get(), 0.0); // Not advanced yet
    }

    #[test]
    fn test_animated_property_advance() {
        let mut prop = AnimatedProperty::with_config(0.0_f32, TransitionConfig::new(1000));
        prop.set(100.0);

        prop.advance(500);
        let value = *prop.get();
        assert!(value > 0.0 && value < 100.0);
        assert!(prop.is_animating());

        prop.advance(500);
        assert_eq!(*prop.get(), 100.0);
        assert!(!prop.is_animating());
    }

    #[test]
    fn test_animated_property_set_immediate() {
        let mut prop = AnimatedProperty::new(0.0_f32);
        prop.set_immediate(50.0);

        assert_eq!(*prop.get(), 50.0);
        assert_eq!(*prop.target(), 50.0);
        assert!(!prop.is_animating());
    }

    #[test]
    fn test_animated_property_with_delay() {
        let mut prop =
            AnimatedProperty::with_config(0.0_f32, TransitionConfig::new(1000).with_delay(500));
        prop.set(100.0);

        // During delay, progress should be 0
        prop.advance(250);
        assert_eq!(prop.progress(), 0.0);
        assert_eq!(*prop.get(), 0.0);

        // After delay, animation begins
        prop.advance(500); // Now 750ms total, 250ms into animation
        assert!(prop.progress() > 0.0);
        assert!(*prop.get() > 0.0);
    }

    #[test]
    fn test_animated_property_f64() {
        let mut prop = AnimatedProperty::with_config(0.0_f64, TransitionConfig::new(1000));
        prop.set(100.0);

        prop.advance(500);
        let value = *prop.get();
        assert!(value > 0.0 && value < 100.0);
    }

    #[test]
    fn test_animated_property_color() {
        let mut prop =
            AnimatedProperty::with_config(crate::Color::BLACK, TransitionConfig::new(1000));
        prop.set(crate::Color::WHITE);

        prop.advance(500);
        let color = *prop.get();
        assert!(color.r > 0.0 && color.r < 1.0);
    }

    #[test]
    fn test_animated_property_point() {
        let mut prop =
            AnimatedProperty::with_config(crate::Point::new(0.0, 0.0), TransitionConfig::new(1000));
        prop.set(crate::Point::new(100.0, 200.0));

        prop.advance(500);
        let point = *prop.get();
        assert!(point.x > 0.0 && point.x < 100.0);
        assert!(point.y > 0.0 && point.y < 200.0);
    }

    #[test]
    fn test_animated_property_size() {
        let mut prop =
            AnimatedProperty::with_config(crate::Size::new(0.0, 0.0), TransitionConfig::new(1000));
        prop.set(crate::Size::new(100.0, 100.0));

        prop.advance(500);
        let size = *prop.get();
        assert!(size.width > 0.0 && size.width < 100.0);
    }

    #[test]
    fn test_animated_property_progress() {
        let mut prop = AnimatedProperty::with_config(0.0_f32, TransitionConfig::new(1000));
        prop.set(100.0);

        assert_eq!(prop.progress(), 0.0);

        prop.advance(250);
        assert!((prop.progress() - 0.25).abs() < 0.001);

        prop.advance(750);
        assert!((prop.progress() - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_animated_property_interrupt() {
        let mut prop = AnimatedProperty::with_config(0.0_f32, TransitionConfig::new(1000));
        prop.set(100.0);

        prop.advance(500);
        let mid_value = *prop.get();
        assert!(mid_value > 0.0);

        // Interrupt with new target
        prop.set(0.0);
        assert!(prop.is_animating());
        assert_eq!(*prop.target(), 0.0);
        // Current value becomes new start
    }

    // =========================================================================
    // SpringConfig Tests
    // =========================================================================

    #[test]
    fn test_spring_config_default() {
        let config = SpringConfig::default();
        assert_eq!(config.stiffness, 100.0);
        assert_eq!(config.damping, 10.0);
        assert_eq!(config.mass, 1.0);
    }

    #[test]
    fn test_spring_config_presets() {
        let gentle = SpringConfig::gentle();
        assert_eq!(gentle.damping, 15.0);

        let bouncy = SpringConfig::bouncy();
        assert_eq!(bouncy.stiffness, 300.0);

        let stiff = SpringConfig::stiff();
        assert_eq!(stiff.stiffness, 500.0);
        assert_eq!(stiff.damping, 30.0);
    }

    // =========================================================================
    // SpringAnimation Tests
    // =========================================================================

    #[test]
    fn test_spring_animation_new() {
        let spring = SpringAnimation::new(0.0);
        assert_eq!(spring.position(), 0.0);
        assert_eq!(spring.velocity(), 0.0);
        assert_eq!(spring.target(), 0.0);
    }

    #[test]
    fn test_spring_animation_set_target() {
        let mut spring = SpringAnimation::new(0.0);
        spring.set_target(100.0);

        assert_eq!(spring.target(), 100.0);
        assert_eq!(spring.position(), 0.0);
    }

    #[test]
    fn test_spring_animation_advance() {
        let mut spring = SpringAnimation::new(0.0);
        spring.set_target(100.0);

        // Advance several steps
        for _ in 0..100 {
            spring.advance_ms(16);
        }

        // Should be close to target
        assert!(spring.position() > 50.0);
    }

    #[test]
    fn test_spring_animation_at_rest() {
        let mut spring = SpringAnimation::new(0.0);
        assert!(spring.is_at_rest()); // At initial position

        spring.set_target(100.0);
        assert!(!spring.is_at_rest());

        // Advance until at rest
        for _ in 0..500 {
            spring.advance_ms(16);
            if spring.is_at_rest() {
                break;
            }
        }

        assert!(spring.is_at_rest());
        assert!((spring.position() - 100.0).abs() < 0.01);
    }

    #[test]
    fn test_spring_animation_set_immediate() {
        let mut spring = SpringAnimation::new(0.0);
        spring.set_target(100.0);
        spring.advance_ms(100);

        spring.set_immediate(50.0);

        assert_eq!(spring.position(), 50.0);
        assert_eq!(spring.target(), 50.0);
        assert_eq!(spring.velocity(), 0.0);
        assert!(spring.is_at_rest());
    }

    #[test]
    fn test_spring_animation_bouncy() {
        let mut spring = SpringAnimation::with_config(0.0, SpringConfig::bouncy());
        spring.set_target(100.0);

        let mut max_position = 0.0_f32;

        // With bouncy spring, position should overshoot
        for _ in 0..200 {
            spring.advance_ms(16);
            max_position = max_position.max(spring.position());
        }

        // Should overshoot past target
        assert!(max_position > 100.0);
    }

    #[test]
    fn test_spring_animation_overdamped() {
        // High damping = critically damped or overdamped
        let config = SpringConfig::new(100.0, 50.0, 1.0);
        let mut spring = SpringAnimation::with_config(0.0, config);
        spring.set_target(100.0);

        let mut max_position = 0.0_f32;

        for _ in 0..500 {
            spring.advance_ms(16);
            max_position = max_position.max(spring.position());
        }

        // Should NOT overshoot with high damping
        assert!(max_position <= 100.1); // Allow small numerical error
    }

    // =========================================================================
    // DataRefreshManager Tests
    // =========================================================================

    #[test]
    fn test_data_refresh_manager_new() {
        let manager = DataRefreshManager::new();
        assert!(manager.tasks().is_empty());
    }

    #[test]
    fn test_data_refresh_manager_default() {
        let manager = DataRefreshManager::default();
        assert!(manager.tasks().is_empty());
    }

    #[test]
    fn test_data_refresh_manager_register() {
        let mut manager = DataRefreshManager::new();
        manager.register("source1", 1000);

        assert_eq!(manager.tasks().len(), 1);
        assert_eq!(manager.tasks()[0].key, "source1");
        assert_eq!(manager.tasks()[0].interval_ms, 1000);
        assert!(manager.tasks()[0].active);
    }

    #[test]
    fn test_data_refresh_manager_register_multiple() {
        let mut manager = DataRefreshManager::new();
        manager.register("source1", 1000);
        manager.register("source2", 2000);
        manager.register("source3", 500);

        assert_eq!(manager.tasks().len(), 3);
    }

    #[test]
    fn test_data_refresh_manager_register_duplicate_updates() {
        let mut manager = DataRefreshManager::new();
        manager.register("source1", 1000);
        manager.register("source1", 2000);

        assert_eq!(manager.tasks().len(), 1);
        assert_eq!(manager.tasks()[0].interval_ms, 2000);
    }

    #[test]
    fn test_data_refresh_manager_unregister() {
        let mut manager = DataRefreshManager::new();
        manager.register("source1", 1000);
        manager.register("source2", 2000);

        manager.unregister("source1");

        assert_eq!(manager.tasks().len(), 1);
        assert_eq!(manager.tasks()[0].key, "source2");
    }

    #[test]
    fn test_data_refresh_manager_unregister_nonexistent() {
        let mut manager = DataRefreshManager::new();
        manager.register("source1", 1000);

        manager.unregister("nonexistent");

        assert_eq!(manager.tasks().len(), 1);
    }

    #[test]
    fn test_data_refresh_manager_pause() {
        let mut manager = DataRefreshManager::new();
        manager.register("source1", 1000);

        manager.pause("source1");

        assert!(!manager.tasks()[0].active);
    }

    #[test]
    fn test_data_refresh_manager_pause_nonexistent() {
        let mut manager = DataRefreshManager::new();
        manager.register("source1", 1000);

        // Should not panic
        manager.pause("nonexistent");

        assert!(manager.tasks()[0].active);
    }

    #[test]
    fn test_data_refresh_manager_resume() {
        let mut manager = DataRefreshManager::new();
        manager.register("source1", 1000);
        manager.pause("source1");

        manager.resume("source1");

        assert!(manager.tasks()[0].active);
    }

    #[test]
    fn test_data_refresh_manager_resume_nonexistent() {
        let mut manager = DataRefreshManager::new();
        manager.register("source1", 1000);
        manager.pause("source1");

        // Should not panic
        manager.resume("nonexistent");

        assert!(!manager.tasks()[0].active);
    }

    #[test]
    fn test_data_refresh_manager_update_initial() {
        let mut manager = DataRefreshManager::new();
        manager.register("source1", 1000);

        // At time 0, elapsed=0, interval=1000, so no refresh yet
        let to_refresh = manager.update(0);
        assert!(to_refresh.is_empty());

        // After interval elapses, refresh should trigger
        let to_refresh = manager.update(1000);
        assert_eq!(to_refresh, vec!["source1"]);
    }

    #[test]
    fn test_data_refresh_manager_update_before_interval() {
        let mut manager = DataRefreshManager::new();
        manager.register("source1", 1000);

        // First refresh at 1000ms
        manager.update(1000);

        // Update before interval elapsed (500ms later)
        let to_refresh = manager.update(1500);

        assert!(to_refresh.is_empty());
    }

    #[test]
    fn test_data_refresh_manager_update_after_interval() {
        let mut manager = DataRefreshManager::new();
        manager.register("source1", 1000);

        // First refresh at 1000ms
        manager.update(1000);

        // Update after interval elapsed (1000ms later)
        let to_refresh = manager.update(2000);

        assert_eq!(to_refresh, vec!["source1"]);
    }

    #[test]
    fn test_data_refresh_manager_update_multiple_sources() {
        let mut manager = DataRefreshManager::new();
        manager.register("fast", 100);
        manager.register("slow", 1000);

        // First refresh for both at their respective intervals
        let to_refresh = manager.update(100);
        assert!(to_refresh.contains(&"fast".to_string()));

        let to_refresh = manager.update(1000);
        assert!(to_refresh.contains(&"slow".to_string()));

        // After 1200ms: fast should refresh (elapsed=200), slow should not (elapsed=200)
        let to_refresh = manager.update(1200);
        assert_eq!(to_refresh.len(), 1);
        assert!(to_refresh.contains(&"fast".to_string()));
    }

    #[test]
    fn test_data_refresh_manager_update_paused_skipped() {
        let mut manager = DataRefreshManager::new();
        manager.register("source1", 1000);
        manager.pause("source1");

        let to_refresh = manager.update(2000);

        assert!(to_refresh.is_empty());
    }

    #[test]
    fn test_data_refresh_manager_force_refresh() {
        let mut manager = DataRefreshManager::new();
        manager.register("source1", 1000);

        // First update at 1000ms triggers refresh (elapsed >= interval)
        manager.update(1000);

        // Update to 1500ms - no refresh yet (elapsed=500 < interval=1000)
        let to_refresh = manager.update(1500);
        assert!(to_refresh.is_empty());

        // Force refresh sets last_refresh_ms to 0
        let result = manager.force_refresh("source1");
        assert!(result);

        // Now update should trigger refresh (elapsed=1500 >= interval=1000)
        let to_refresh = manager.update(1500);
        assert_eq!(to_refresh, vec!["source1"]);
    }

    #[test]
    fn test_data_refresh_manager_force_refresh_nonexistent() {
        let mut manager = DataRefreshManager::new();
        manager.register("source1", 1000);

        let result = manager.force_refresh("nonexistent");

        assert!(!result);
    }

    #[test]
    fn test_data_refresh_manager_get_task() {
        let mut manager = DataRefreshManager::new();
        manager.register("source1", 1000);

        let task = manager.get_task("source1");
        assert!(task.is_some());
        assert_eq!(task.unwrap().key, "source1");

        let task = manager.get_task("nonexistent");
        assert!(task.is_none());
    }

    #[test]
    fn test_data_refresh_manager_is_due() {
        let mut manager = DataRefreshManager::new();
        manager.register("source1", 1000);

        // At time 0, elapsed=0, interval=1000, not due yet
        manager.update(0);
        assert!(!manager.is_due("source1"));

        // At time 500, still not due
        manager.update(500);
        assert!(!manager.is_due("source1"));

        // At time 999, still not due (elapsed=999 < 1000)
        manager.update(999);
        assert!(!manager.is_due("source1"));

        // At time 1000, should be due (elapsed >= interval)
        // But update() triggers and resets, so we need to force check differently
        // Use force_refresh to reset and then check is_due
        manager.update(1000); // Triggers refresh, sets last_refresh_ms=1000
        assert!(!manager.is_due("source1")); // Just refreshed

        // Advance time without triggering refresh
        manager.update(1500);
        assert!(!manager.is_due("source1")); // elapsed=500 < 1000

        // Now check at 2000 before update
        // We need to manually check - but update also triggers, so this is tricky
        // The is_due check uses stored current_time_ms which is 1500
    }

    #[test]
    fn test_data_refresh_manager_is_due_paused() {
        let mut manager = DataRefreshManager::new();
        manager.register("source1", 1000);
        manager.pause("source1");

        manager.update(2000);

        assert!(!manager.is_due("source1"));
    }

    #[test]
    fn test_data_refresh_manager_is_due_nonexistent() {
        let manager = DataRefreshManager::new();
        assert!(!manager.is_due("nonexistent"));
    }

    #[test]
    fn test_data_refresh_manager_time_until_refresh() {
        let mut manager = DataRefreshManager::new();
        manager.register("source1", 1000);

        // First update at time 1000 triggers refresh
        manager.update(1000);

        // At time 1000, just refreshed, 1000ms until next
        assert_eq!(manager.time_until_refresh("source1"), Some(1000));

        // At time 1500, 500ms until next
        manager.update(1500);
        // last_refresh_ms is 1000, current_time is 1500, elapsed = 500
        // time_until = 1000 - 500 = 500
        assert_eq!(manager.time_until_refresh("source1"), Some(500));
    }

    #[test]
    fn test_data_refresh_manager_time_until_refresh_paused() {
        let mut manager = DataRefreshManager::new();
        manager.register("source1", 1000);
        manager.pause("source1");

        manager.update(500);

        assert_eq!(manager.time_until_refresh("source1"), Some(u64::MAX));
    }

    #[test]
    fn test_data_refresh_manager_time_until_refresh_nonexistent() {
        let manager = DataRefreshManager::new();
        assert_eq!(manager.time_until_refresh("nonexistent"), None);
    }

    #[test]
    fn test_data_refresh_manager_saturating_arithmetic() {
        let mut manager = DataRefreshManager::new();
        manager.register("source1", 1000);

        // Update with very large time, shouldn't panic
        manager.update(u64::MAX - 1);
        let to_refresh = manager.update(u64::MAX);

        // Should handle overflow gracefully
        assert!(to_refresh.is_empty() || to_refresh.len() == 1);
    }

    #[test]
    fn test_data_refresh_manager_reactivate_updates_interval() {
        let mut manager = DataRefreshManager::new();
        manager.register("source1", 1000);
        manager.pause("source1");

        // Re-register while paused should reactivate with new interval
        manager.register("source1", 500);

        assert!(manager.tasks()[0].active);
        assert_eq!(manager.tasks()[0].interval_ms, 500);
    }

    #[test]
    fn test_data_refresh_manager_multiple_refresh_cycles() {
        let mut manager = DataRefreshManager::new();
        manager.register("source1", 100);

        let mut refresh_count = 0;

        for time in (0..1000).step_by(50) {
            let to_refresh = manager.update(time as u64);
            refresh_count += to_refresh.len();
        }

        // With 100ms interval over 1000ms, should refresh ~10 times
        // (at 0, 100, 200, 300, 400, 500, 600, 700, 800, 900)
        assert!(refresh_count >= 9 && refresh_count <= 11);
    }
}
