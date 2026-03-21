#[allow(clippy::unwrap_used, clippy::disallowed_methods)]
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
        // EaseInQuad: value at t=0.5 is below midpoint.
        assert!(EasingFunction::EaseInQuad.apply(0.5) < 0.5);
        // EaseOutQuad: value at t=0.5 is above midpoint.
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

        // At 1200ms: high-frequency source refreshes, low-frequency does not.
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
        assert!((9..=11).contains(&refresh_count));
    }
}
