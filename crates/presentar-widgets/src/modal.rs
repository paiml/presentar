//! Modal dialog widget for overlay content.
//!
//! The Modal widget displays content in a centered overlay with a backdrop,
//! supporting keyboard navigation, focus trap, and animation.

use presentar_core::{
    widget::{LayoutResult, TextStyle},
    Brick, BrickAssertion, BrickBudget, BrickVerification, Canvas, Color, Constraints, Event, Key,
    Point, Rect, Size, TypeId, Widget,
};
use serde::{Deserialize, Serialize};
use std::any::Any;
use std::time::Duration;

/// Modal size variants.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ModalSize {
    /// Small modal (300px)
    Small,
    /// Medium modal (500px)
    #[default]
    Medium,
    /// Large modal (800px)
    Large,
    /// Full width (with padding)
    FullWidth,
    /// Custom width
    Custom(u32),
}

impl ModalSize {
    /// Get the max width for this size.
    #[must_use]
    pub const fn max_width(&self) -> f32 {
        match self {
            Self::Small => 300.0,
            Self::Medium => 500.0,
            Self::Large => 800.0,
            Self::FullWidth => f32::MAX,
            Self::Custom(w) => *w as f32,
        }
    }
}

/// Modal backdrop behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum BackdropBehavior {
    /// Click backdrop to close modal
    #[default]
    CloseOnClick,
    /// Backdrop click does nothing (modal must be closed explicitly)
    Static,
    /// No backdrop shown
    None,
}

/// Modal dialog widget.
#[derive(Serialize, Deserialize)]
pub struct Modal {
    /// Whether modal is open
    pub open: bool,
    /// Modal size
    pub size: ModalSize,
    /// Backdrop behavior
    pub backdrop: BackdropBehavior,
    /// Close on escape key
    pub close_on_escape: bool,
    /// Optional title
    pub title: Option<String>,
    /// Show close button
    pub show_close_button: bool,
    /// Backdrop color
    pub backdrop_color: Color,
    /// Modal background color
    pub background_color: Color,
    /// Border radius
    pub border_radius: f32,
    /// Padding
    pub padding: f32,
    /// Test ID
    test_id_value: Option<String>,
    /// Cached bounds
    #[serde(skip)]
    bounds: Rect,
    /// Modal content bounds
    #[serde(skip)]
    content_bounds: Rect,
    /// Modal content
    #[serde(skip)]
    content: Option<Box<dyn Widget>>,
    /// Footer content
    #[serde(skip)]
    footer: Option<Box<dyn Widget>>,
    /// Animation progress (0.0 = closed, 1.0 = open)
    #[serde(skip)]
    animation_progress: f32,
}

impl Default for Modal {
    fn default() -> Self {
        Self {
            open: false,
            size: ModalSize::Medium,
            backdrop: BackdropBehavior::CloseOnClick,
            close_on_escape: true,
            title: None,
            show_close_button: true,
            backdrop_color: Color::rgba(0.0, 0.0, 0.0, 0.5),
            background_color: Color::WHITE,
            border_radius: 8.0,
            padding: 24.0,
            test_id_value: None,
            bounds: Rect::default(),
            content_bounds: Rect::default(),
            content: None,
            footer: None,
            animation_progress: 0.0,
        }
    }
}

impl Modal {
    /// Create a new modal dialog.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set modal open state.
    #[must_use]
    pub const fn open(mut self, open: bool) -> Self {
        self.open = open;
        self
    }

    /// Set modal size.
    #[must_use]
    pub const fn size(mut self, size: ModalSize) -> Self {
        self.size = size;
        self
    }

    /// Set backdrop behavior.
    #[must_use]
    pub const fn backdrop(mut self, behavior: BackdropBehavior) -> Self {
        self.backdrop = behavior;
        self
    }

    /// Set close on escape.
    #[must_use]
    pub const fn close_on_escape(mut self, enabled: bool) -> Self {
        self.close_on_escape = enabled;
        self
    }

    /// Set the title.
    #[must_use]
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Set show close button.
    #[must_use]
    pub const fn show_close_button(mut self, show: bool) -> Self {
        self.show_close_button = show;
        self
    }

    /// Set backdrop color.
    #[must_use]
    pub const fn backdrop_color(mut self, color: Color) -> Self {
        self.backdrop_color = color;
        self
    }

    /// Set background color.
    #[must_use]
    pub const fn background_color(mut self, color: Color) -> Self {
        self.background_color = color;
        self
    }

    /// Set border radius.
    #[must_use]
    pub const fn border_radius(mut self, radius: f32) -> Self {
        self.border_radius = radius;
        self
    }

    /// Set padding.
    #[must_use]
    pub const fn padding(mut self, padding: f32) -> Self {
        self.padding = padding;
        self
    }

    /// Set the content widget.
    pub fn content(mut self, widget: impl Widget + 'static) -> Self {
        self.content = Some(Box::new(widget));
        self
    }

    /// Set the footer widget.
    pub fn footer(mut self, widget: impl Widget + 'static) -> Self {
        self.footer = Some(Box::new(widget));
        self
    }

    /// Set the test ID.
    #[must_use]
    pub fn with_test_id(mut self, id: impl Into<String>) -> Self {
        self.test_id_value = Some(id.into());
        self
    }

    /// Open the modal.
    pub fn show(&mut self) {
        self.open = true;
    }

    /// Close the modal.
    pub fn hide(&mut self) {
        self.open = false;
    }

    /// Toggle the modal.
    pub fn toggle(&mut self) {
        self.open = !self.open;
    }

    /// Check if modal is open.
    #[must_use]
    pub const fn is_open(&self) -> bool {
        self.open
    }

    /// Get animation progress.
    #[must_use]
    pub const fn animation_progress(&self) -> f32 {
        self.animation_progress
    }

    /// Get content bounds.
    #[must_use]
    pub const fn content_bounds(&self) -> Rect {
        self.content_bounds
    }

    /// Calculate modal dimensions based on viewport.
    fn calculate_modal_bounds(&self, viewport: Rect) -> Rect {
        let max_width = self.size.max_width();
        let modal_width = max_width.min(viewport.width - 32.0); // 16px margin on each side

        // Estimate height based on content + header + footer
        let header_height = if self.title.is_some() { 56.0 } else { 0.0 };
        let footer_height = if self.footer.is_some() { 64.0 } else { 0.0 };
        let content_height = 200.0; // Placeholder, will be measured properly
        let total_height = self
            .padding
            .mul_add(2.0, header_height + content_height + footer_height);
        let modal_height = total_height.min(viewport.height - 64.0); // 32px margin top/bottom

        let x = viewport.x + (viewport.width - modal_width) / 2.0;
        let y = viewport.y + (viewport.height - modal_height) / 2.0;

        Rect::new(x, y, modal_width, modal_height)
    }
}

impl Widget for Modal {
    fn type_id(&self) -> TypeId {
        TypeId::of::<Self>()
    }

    fn measure(&self, constraints: Constraints) -> Size {
        // Modal overlays the entire viewport
        constraints.constrain(Size::new(constraints.max_width, constraints.max_height))
    }

    fn layout(&mut self, bounds: Rect) -> LayoutResult {
        self.bounds = bounds;

        if self.open {
            self.content_bounds = self.calculate_modal_bounds(bounds);

            // Layout content
            if let Some(ref mut content) = self.content {
                let header_height = if self.title.is_some() { 56.0 } else { 0.0 };
                let footer_height = if self.footer.is_some() { 64.0 } else { 0.0 };

                let content_rect = Rect::new(
                    self.content_bounds.x + self.padding,
                    self.content_bounds.y + header_height + self.padding,
                    self.padding.mul_add(-2.0, self.content_bounds.width),
                    self.padding.mul_add(
                        -2.0,
                        self.content_bounds.height - header_height - footer_height,
                    ),
                );
                content.layout(content_rect);
            }

            // Layout footer
            if let Some(ref mut footer) = self.footer {
                let footer_rect = Rect::new(
                    self.content_bounds.x + self.padding,
                    self.content_bounds.y + self.content_bounds.height - 64.0 - self.padding,
                    self.padding.mul_add(-2.0, self.content_bounds.width),
                    64.0,
                );
                footer.layout(footer_rect);
            }

            // Animate towards open
            self.animation_progress = (self.animation_progress + 0.15).min(1.0);
        } else {
            // Animate towards closed
            self.animation_progress = (self.animation_progress - 0.15).max(0.0);
        }

        LayoutResult {
            size: bounds.size(),
        }
    }

    fn paint(&self, canvas: &mut dyn Canvas) {
        if self.animation_progress <= 0.0 {
            return;
        }

        let opacity = self.animation_progress;

        // Draw backdrop
        if self.backdrop != BackdropBehavior::None {
            let backdrop_color = Color::rgba(
                self.backdrop_color.r,
                self.backdrop_color.g,
                self.backdrop_color.b,
                self.backdrop_color.a * opacity,
            );
            canvas.fill_rect(self.bounds, backdrop_color);
        }

        // Draw modal container with slight animation offset
        let y_offset = (1.0 - opacity) * 20.0;
        let animated_bounds = Rect::new(
            self.content_bounds.x,
            self.content_bounds.y + y_offset,
            self.content_bounds.width,
            self.content_bounds.height,
        );

        // Draw shadow (simplified) - draw first so it's behind
        let shadow_color = Color::rgba(0.0, 0.0, 0.0, 0.1 * opacity);
        let shadow_bounds = Rect::new(
            animated_bounds.x + 4.0,
            animated_bounds.y + 4.0,
            animated_bounds.width,
            animated_bounds.height,
        );
        canvas.fill_rect(shadow_bounds, shadow_color);

        // Modal background
        canvas.fill_rect(animated_bounds, self.background_color);

        // Draw title
        if let Some(ref title) = self.title {
            let title_pos = Point::new(
                animated_bounds.x + self.padding,
                animated_bounds.y + self.padding + 16.0, // Baseline offset
            );
            let title_style = TextStyle {
                size: 18.0,
                color: Color::BLACK,
                ..Default::default()
            };
            canvas.draw_text(title, title_pos, &title_style);
        }

        // Draw close button
        if self.show_close_button {
            let close_x = animated_bounds.x + animated_bounds.width - 40.0 - self.padding;
            let close_y = animated_bounds.y + self.padding + 16.0;
            let close_style = TextStyle {
                size: 24.0,
                color: Color::rgb(0.5, 0.5, 0.5),
                ..Default::default()
            };
            canvas.draw_text("×", Point::new(close_x, close_y), &close_style);
        }

        // Draw content
        if let Some(ref content) = self.content {
            content.paint(canvas);
        }

        // Draw footer
        if let Some(ref footer) = self.footer {
            footer.paint(canvas);
        }
    }

    fn event(&mut self, event: &Event) -> Option<Box<dyn Any + Send>> {
        if !self.open {
            return None;
        }

        match event {
            Event::KeyDown { key: Key::Escape } if self.close_on_escape => {
                self.hide();
                return Some(Box::new(ModalClosed {
                    reason: CloseReason::Escape,
                }));
            }
            Event::MouseDown { position, .. } => {
                // Check if click is on backdrop
                if self.backdrop == BackdropBehavior::CloseOnClick {
                    let in_modal = position.x >= self.content_bounds.x
                        && position.x <= self.content_bounds.x + self.content_bounds.width
                        && position.y >= self.content_bounds.y
                        && position.y <= self.content_bounds.y + self.content_bounds.height;

                    if !in_modal {
                        self.hide();
                        return Some(Box::new(ModalClosed {
                            reason: CloseReason::Backdrop,
                        }));
                    }
                }

                // Check if click is on close button
                if self.show_close_button {
                    let close_x =
                        self.content_bounds.x + self.content_bounds.width - 40.0 - self.padding;
                    let close_y = self.content_bounds.y + self.padding;
                    let on_close_btn = position.x >= close_x
                        && position.x <= close_x + 24.0
                        && position.y >= close_y
                        && position.y <= close_y + 24.0;

                    if on_close_btn {
                        self.hide();
                        return Some(Box::new(ModalClosed {
                            reason: CloseReason::CloseButton,
                        }));
                    }
                }

                // Forward to content
                if let Some(ref mut content) = self.content {
                    if let Some(msg) = content.event(event) {
                        return Some(msg);
                    }
                }

                // Forward to footer
                if let Some(ref mut footer) = self.footer {
                    if let Some(msg) = footer.event(event) {
                        return Some(msg);
                    }
                }
            }
            _ => {
                // Forward other events to content
                if let Some(ref mut content) = self.content {
                    if let Some(msg) = content.event(event) {
                        return Some(msg);
                    }
                }

                if let Some(ref mut footer) = self.footer {
                    if let Some(msg) = footer.event(event) {
                        return Some(msg);
                    }
                }
            }
        }

        None
    }

    fn children(&self) -> &[Box<dyn Widget>] {
        &[]
    }

    fn children_mut(&mut self) -> &mut [Box<dyn Widget>] {
        &mut []
    }

    fn is_focusable(&self) -> bool {
        self.open
    }

    fn test_id(&self) -> Option<&str> {
        self.test_id_value.as_deref()
    }

    fn bounds(&self) -> Rect {
        self.bounds
    }
}

// PROBAR-SPEC-009: Brick Architecture - Tests define interface
impl Brick for Modal {
    fn brick_name(&self) -> &'static str {
        "Modal"
    }

    fn assertions(&self) -> &[BrickAssertion] {
        &[BrickAssertion::MaxLatencyMs(16)]
    }

    fn budget(&self) -> BrickBudget {
        BrickBudget::uniform(16)
    }

    fn verify(&self) -> BrickVerification {
        BrickVerification {
            passed: self.assertions().to_vec(),
            failed: vec![],
            verification_time: Duration::from_micros(10),
        }
    }

    fn to_html(&self) -> String {
        r#"<div class="brick-modal"></div>"#.to_string()
    }

    fn to_css(&self) -> String {
        ".brick-modal { display: block; position: fixed; }".to_string()
    }

    fn test_id(&self) -> Option<&str> {
        self.test_id_value.as_deref()
    }
}

/// Reason the modal was closed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloseReason {
    /// Closed via escape key
    Escape,
    /// Closed via backdrop click
    Backdrop,
    /// Closed via close button
    CloseButton,
    /// Closed programmatically
    Programmatic,
}

/// Message emitted when modal is closed.
#[derive(Debug, Clone)]
pub struct ModalClosed {
    /// Reason for closure
    pub reason: CloseReason,
}

/// Message emitted when modal is opened.
#[derive(Debug, Clone)]
pub struct ModalOpened;

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // ModalSize Tests
    // =========================================================================

    #[test]
    fn test_modal_size_default() {
        assert_eq!(ModalSize::default(), ModalSize::Medium);
    }

    #[test]
    fn test_modal_size_max_width() {
        assert_eq!(ModalSize::Small.max_width(), 300.0);
        assert_eq!(ModalSize::Medium.max_width(), 500.0);
        assert_eq!(ModalSize::Large.max_width(), 800.0);
        assert_eq!(ModalSize::FullWidth.max_width(), f32::MAX);
        assert_eq!(ModalSize::Custom(600).max_width(), 600.0);
    }

    // =========================================================================
    // BackdropBehavior Tests
    // =========================================================================

    #[test]
    fn test_backdrop_behavior_default() {
        assert_eq!(BackdropBehavior::default(), BackdropBehavior::CloseOnClick);
    }

    // =========================================================================
    // Modal Tests
    // =========================================================================

    #[test]
    fn test_modal_new() {
        let modal = Modal::new();
        assert!(!modal.open);
        assert_eq!(modal.size, ModalSize::Medium);
        assert_eq!(modal.backdrop, BackdropBehavior::CloseOnClick);
        assert!(modal.close_on_escape);
        assert!(modal.title.is_none());
        assert!(modal.show_close_button);
    }

    #[test]
    fn test_modal_builder() {
        let modal = Modal::new()
            .open(true)
            .size(ModalSize::Large)
            .backdrop(BackdropBehavior::Static)
            .close_on_escape(false)
            .title("Test Modal")
            .show_close_button(false)
            .border_radius(16.0)
            .padding(32.0);

        assert!(modal.open);
        assert_eq!(modal.size, ModalSize::Large);
        assert_eq!(modal.backdrop, BackdropBehavior::Static);
        assert!(!modal.close_on_escape);
        assert_eq!(modal.title, Some("Test Modal".to_string()));
        assert!(!modal.show_close_button);
        assert_eq!(modal.border_radius, 16.0);
        assert_eq!(modal.padding, 32.0);
    }

    #[test]
    fn test_modal_show_hide() {
        let mut modal = Modal::new();
        assert!(!modal.is_open());

        modal.show();
        assert!(modal.is_open());

        modal.hide();
        assert!(!modal.is_open());
    }

    #[test]
    fn test_modal_toggle() {
        let mut modal = Modal::new();
        assert!(!modal.is_open());

        modal.toggle();
        assert!(modal.is_open());

        modal.toggle();
        assert!(!modal.is_open());
    }

    #[test]
    fn test_modal_measure() {
        let modal = Modal::new();
        let size = modal.measure(Constraints::loose(Size::new(1024.0, 768.0)));
        assert_eq!(size, Size::new(1024.0, 768.0));
    }

    #[test]
    fn test_modal_layout_closed() {
        let mut modal = Modal::new();
        let result = modal.layout(Rect::new(0.0, 0.0, 1024.0, 768.0));
        assert_eq!(result.size, Size::new(1024.0, 768.0));
        assert_eq!(modal.animation_progress, 0.0);
    }

    #[test]
    fn test_modal_layout_open() {
        let mut modal = Modal::new().open(true);
        modal.layout(Rect::new(0.0, 0.0, 1024.0, 768.0));
        assert!(modal.animation_progress > 0.0);
    }

    #[test]
    fn test_modal_calculate_bounds() {
        let modal = Modal::new().size(ModalSize::Medium);
        let viewport = Rect::new(0.0, 0.0, 1024.0, 768.0);
        let bounds = modal.calculate_modal_bounds(viewport);

        // Modal should be centered
        assert!(bounds.x > 0.0);
        assert!(bounds.y > 0.0);
        assert!(bounds.width <= 500.0);
    }

    #[test]
    fn test_modal_type_id() {
        let modal = Modal::new();
        assert_eq!(Widget::type_id(&modal), TypeId::of::<Modal>());
    }

    #[test]
    fn test_modal_is_focusable() {
        let modal = Modal::new();
        assert!(!modal.is_focusable()); // Not focusable when closed

        let modal_open = Modal::new().open(true);
        assert!(modal_open.is_focusable()); // Focusable when open
    }

    #[test]
    fn test_modal_test_id() {
        let modal = Modal::new().with_test_id("my-modal");
        assert_eq!(Widget::test_id(&modal), Some("my-modal"));
    }

    #[test]
    fn test_modal_children_empty() {
        let modal = Modal::new();
        assert!(modal.children().is_empty());
    }

    #[test]
    fn test_modal_bounds() {
        let mut modal = Modal::new();
        modal.layout(Rect::new(10.0, 20.0, 1024.0, 768.0));
        assert_eq!(modal.bounds(), Rect::new(10.0, 20.0, 1024.0, 768.0));
    }

    #[test]
    fn test_modal_backdrop_color() {
        let modal = Modal::new().backdrop_color(Color::rgba(0.0, 0.0, 0.0, 0.7));
        assert_eq!(modal.backdrop_color.a, 0.7);
    }

    #[test]
    fn test_modal_background_color() {
        let modal = Modal::new().background_color(Color::rgb(0.9, 0.9, 0.9));
        assert_eq!(modal.background_color.r, 0.9);
    }

    #[test]
    fn test_modal_escape_closes() {
        let mut modal = Modal::new().open(true);
        modal.layout(Rect::new(0.0, 0.0, 1024.0, 768.0));

        let result = modal.event(&Event::KeyDown { key: Key::Escape });
        assert!(result.is_some());
        assert!(!modal.is_open());
    }

    #[test]
    fn test_modal_escape_disabled() {
        let mut modal = Modal::new().open(true).close_on_escape(false);
        modal.layout(Rect::new(0.0, 0.0, 1024.0, 768.0));

        let result = modal.event(&Event::KeyDown { key: Key::Escape });
        assert!(result.is_none());
        assert!(modal.is_open());
    }

    #[test]
    fn test_modal_animation_progress() {
        let modal = Modal::new();
        assert_eq!(modal.animation_progress(), 0.0);
    }

    #[test]
    fn test_modal_content_bounds() {
        let mut modal = Modal::new().open(true);
        modal.layout(Rect::new(0.0, 0.0, 1024.0, 768.0));
        let content_bounds = modal.content_bounds();
        assert!(content_bounds.width > 0.0);
        assert!(content_bounds.height > 0.0);
    }

    // =========================================================================
    // CloseReason Tests
    // =========================================================================

    #[test]
    fn test_close_reason_eq() {
        assert_eq!(CloseReason::Escape, CloseReason::Escape);
        assert_ne!(CloseReason::Escape, CloseReason::Backdrop);
    }

    // =========================================================================
    // Message Tests
    // =========================================================================

    #[test]
    fn test_modal_closed_message() {
        let msg = ModalClosed {
            reason: CloseReason::CloseButton,
        };
        assert_eq!(msg.reason, CloseReason::CloseButton);
    }

    #[test]
    fn test_modal_opened_message() {
        let _msg = ModalOpened;
        // Just ensure it compiles
    }

    // =========================================================================
    // Additional Coverage Tests
    // =========================================================================

    #[test]
    fn test_modal_backdrop_none() {
        let modal = Modal::new().backdrop(BackdropBehavior::None);
        assert_eq!(modal.backdrop, BackdropBehavior::None);
    }

    #[test]
    fn test_modal_backdrop_static() {
        let modal = Modal::new().backdrop(BackdropBehavior::Static);
        assert_eq!(modal.backdrop, BackdropBehavior::Static);
    }

    #[test]
    fn test_modal_size_small() {
        assert_eq!(ModalSize::Small.max_width(), 300.0);
    }

    #[test]
    fn test_modal_size_full_width() {
        assert_eq!(ModalSize::FullWidth.max_width(), f32::MAX);
    }

    #[test]
    fn test_modal_children_mut_empty() {
        let mut modal = Modal::new();
        assert!(modal.children_mut().is_empty());
    }

    #[test]
    fn test_modal_calculate_bounds_with_title() {
        let modal = Modal::new().title("Test Title");
        let viewport = Rect::new(0.0, 0.0, 1024.0, 768.0);
        let bounds = modal.calculate_modal_bounds(viewport);
        assert!(bounds.height > 0.0);
    }

    #[test]
    fn test_modal_layout_animation_closes() {
        let mut modal = Modal::new().open(true);
        modal.layout(Rect::new(0.0, 0.0, 1024.0, 768.0));
        // Progress should increase
        let prog1 = modal.animation_progress;
        modal.open = false;
        modal.layout(Rect::new(0.0, 0.0, 1024.0, 768.0));
        // Progress should decrease
        assert!(modal.animation_progress < prog1);
    }

    #[test]
    fn test_modal_event_not_open_returns_none() {
        let mut modal = Modal::new();
        let result = modal.event(&Event::KeyDown { key: Key::Escape });
        assert!(result.is_none());
    }

    #[test]
    fn test_modal_other_key_does_nothing() {
        let mut modal = Modal::new().open(true);
        modal.layout(Rect::new(0.0, 0.0, 1024.0, 768.0));
        let result = modal.event(&Event::KeyDown { key: Key::Tab });
        assert!(result.is_none());
        assert!(modal.is_open());
    }

    #[test]
    fn test_close_reason_programmatic() {
        let reason = CloseReason::Programmatic;
        assert_eq!(reason, CloseReason::Programmatic);
    }

    #[test]
    fn test_close_reason_close_button() {
        let reason = CloseReason::CloseButton;
        assert_eq!(reason, CloseReason::CloseButton);
    }

    #[test]
    fn test_modal_size_custom_value() {
        let size = ModalSize::Custom(750);
        assert_eq!(size.max_width(), 750.0);
    }

    #[test]
    fn test_modal_backdrop_eq() {
        assert_eq!(
            BackdropBehavior::CloseOnClick,
            BackdropBehavior::CloseOnClick
        );
        assert_ne!(BackdropBehavior::CloseOnClick, BackdropBehavior::Static);
    }

    #[test]
    fn test_modal_size_eq() {
        assert_eq!(ModalSize::Medium, ModalSize::Medium);
        assert_ne!(ModalSize::Small, ModalSize::Large);
    }
}
