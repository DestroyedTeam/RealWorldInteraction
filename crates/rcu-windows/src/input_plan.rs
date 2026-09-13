//! Portable input planning and dispatch sequencing shared with native Windows.
//! A batch is one balanced gesture, never a whole text stream or retry queue.
mod keyboard;
#[cfg(test)]
use keyboard::chord;
use keyboard::chord_with_policy;
use rcu_core::{Intent, Result, fault, operation::checkpoint};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Batch {
    Click { right: bool },
    Unicode(Vec<u16>),
    Chord(Vec<u16>),
    Wheel(i32),
}
#[derive(Debug)]
pub(crate) struct Plan {
    pub batches: Vec<Batch>,
    pub keyboard: bool,
    pub text: bool,
    pub drag: Option<rcu_core::trajectory::DragPath>,
}

#[cfg(test)]
fn plan(intent: &Intent, value: Option<&str>) -> Result<Plan> {
    plan_with_policy(intent, value, false)
}

pub(crate) fn plan_with_policy(
    intent: &Intent,
    value: Option<&str>,
    desktop_keys: bool,
) -> Result<Plan> {
    intent.validate(value)?;
    let need_value = || value.ok_or_else(|| fault("value_required", "Supply the action value."));
    let mut keyboard = false;
    let mut text = false;
    let batches = match intent {
        Intent::Drag(path) => {
            return Ok(Plan {
                batches: vec![],
                keyboard: false,
                text: false,
                drag: Some(path.clone()),
            });
        }
        Intent::Click | Intent::DoubleClick | Intent::ContextMenu => {
            if value.is_some() {
                return Err(fault(
                    "invalid_input_value",
                    "Pointer clicks take no value.",
                ));
            }
            vec![
                Batch::Click {
                    right: matches!(intent, Intent::ContextMenu)
                };
                if matches!(intent, Intent::DoubleClick) {
                    2
                } else {
                    1
                }
            ]
        }
        Intent::Text => {
            keyboard = true;
            text = true;
            let value = need_value()?;
            if value.is_empty()
                || value.encode_utf16().count() > 1024
                || value.chars().any(|c| c.is_control())
            {
                return Err(fault(
                    "invalid_text",
                    "Use 1..1024 UTF-16 units of plain text; send Enter/Tab separately as grounded key_chord actions.",
                ));
            }
            value
                .chars()
                .map(|c| Batch::Unicode(c.encode_utf16(&mut [0; 2]).to_vec()))
                .collect()
        }
        Intent::KeyChord => {
            keyboard = true;
            vec![Batch::Chord(chord_with_policy(
                need_value()?,
                desktop_keys,
            )?)]
        }
        Intent::Scroll => {
            let units = need_value()?.parse::<i32>().map_err(|_| {
                fault(
                    "invalid_scroll",
                    "Windows scroll is signed wheel units, multiples of 120, bounded to +/-1200.",
                )
            })?;
            if units == 0 || units.unsigned_abs() > 1200 || units % 120 != 0 {
                return Err(fault(
                    "invalid_scroll",
                    "Windows scroll is signed wheel units, multiples of 120, bounded to +/-1200.",
                ));
            }
            vec![Batch::Wheel(units)]
        }
    };
    Ok(Plan {
        batches,
        keyboard,
        text,
        drag: None,
    })
}

/// The native sink uses this same portable order. A blocking document read must
/// finish before exact UIA scope and final window/session/layout/user guards.
/// This is optimistic revalidation, not an OS-atomic compare-and-write.
pub(crate) trait GuardPhases {
    fn document_precondition(&mut self) -> Result<()>;
    fn exact_input_scope(&mut self) -> Result<()>;
    fn final_native_state(&mut self) -> Result<()>;
}
pub(crate) fn guard_before_batch(guard: &mut impl GuardPhases) -> Result<()> {
    checkpoint("windows_input_guard_begin")?;
    guard.document_precondition()?;
    checkpoint("windows_input_document_returned")?;
    guard.exact_input_scope()?;
    checkpoint("windows_input_exact_scope_returned")?;
    guard.final_native_state()?;
    // Includes the monotonic cooperative lease deadline after all native guards.
    checkpoint("windows_input_guard_ready")
}

pub(crate) trait Sink {
    fn guard(&mut self) -> Result<()>;
    fn send(&mut self, batch: &Batch) -> Result<()>;
    fn pause(&mut self) {}
}
pub(crate) fn execute(plan: &Plan, sink: &mut impl Sink) -> Result<()> {
    if plan.drag.is_some() {
        return Err(fault(
            "wrong_input_executor",
            "A drag requires the release-owning trajectory executor.",
        ));
    }
    for (index, batch) in plan.batches.iter().enumerate() {
        if index != 0 {
            sink.pause();
        }
        checkpoint("windows_input_batch")?;
        sink.guard()?;
        // Recheck cancellation after potentially blocking identity/UIA guards.
        checkpoint("windows_input_dispatch")?;
        sink.send(batch)?;
    }
    Ok(())
}

pub(crate) fn absolute_axis(value: f64, origin: i32, extent: i32) -> Result<i32> {
    if !value.is_finite() || extent <= 1 {
        return Err(fault(
            "invalid_input_coordinates",
            "Physical desktop coordinates unavailable.",
        ));
    }
    let pixel = value.round() - f64::from(origin);
    if pixel < 0.0 || pixel >= f64::from(extent) {
        return Err(fault(
            "invalid_input_coordinates",
            "Point is outside the physical virtual desktop.",
        ));
    }
    Ok((pixel * 65535.0 / f64::from(extent - 1)).round() as i32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rcu_core::operation::{self, Operation, Registry};
    #[test]
    fn unicode_batches_are_balanced_scalars_not_split_surrogates() {
        let p = plan(&Intent::Text, Some("A文🦀")).unwrap();
        assert_eq!(
            p.batches,
            vec![
                Batch::Unicode(vec![65]),
                Batch::Unicode(vec![25991]),
                Batch::Unicode(vec![0xd83e, 0xdd80])
            ]
        );
        assert!(p.keyboard && p.text);
        for s in ["", "a\nb", "\t", "\0"] {
            assert!(plan(&Intent::Text, Some(s)).is_err());
        }
        assert!(plan(&Intent::Text, Some(&"x".repeat(1025))).is_err());
    }
    #[test]
    fn gestures_and_chords_are_bounded_and_system_chords_rejected() {
        assert_eq!(
            plan(&Intent::KeyChord, Some("Ctrl+Shift+S"))
                .unwrap()
                .batches,
            vec![Batch::Chord(vec![0x11, 0x10, 0x53])]
        );
        for s in [
            "Alt+Tab",
            "Ctrl+Alt+Delete",
            "Ctrl+Escape",
            "Win+R",
            "Ctrl+Ctrl+S",
            "",
        ] {
            assert!(plan(&Intent::KeyChord, Some(s)).is_err());
        }
        assert_eq!(plan(&Intent::DoubleClick, None).unwrap().batches.len(), 2);
        assert!(plan(&Intent::Click, Some("ignored")).is_err());
        for s in ["0", "1", "1201", "-2000", "x"] {
            assert!(plan(&Intent::Scroll, Some(s)).is_err());
        }
        assert!(plan(&Intent::Scroll, Some("-120")).is_ok());
    }
    #[test]
    fn all_function_keys_are_planned_without_enabling_system_chords() {
        for n in 1..=24 {
            assert_eq!(chord(&format!("F{n}")).unwrap(), vec![0x70 + n - 1]);
        }
        assert_eq!(chord("Ctrl+F5").unwrap(), vec![0x11, 0x74]);
        for key in [
            "F0",
            "F25",
            "F05",
            "F-1",
            "F999999",
            "Alt+Tab",
            "Win+F5",
            "Ctrl+Alt+Delete",
        ] {
            assert!(chord(key).is_err(), "{key}");
        }
    }
    #[test]
    fn desktop_policy_plans_balanced_shell_keys_without_enabling_secure_attention() {
        for (chord, keys) in [
            ("Win+E", vec![0x5b, 0x45]),
            ("Win+R", vec![0x5b, 0x52]),
            ("Win", vec![0x5b]),
            ("Alt+Tab", vec![0x12, 0x09]),
            ("Ctrl+Escape", vec![0x11, 0x1b]),
        ] {
            let p = plan_with_policy(&Intent::KeyChord, Some(chord), true).unwrap();
            assert!(p.keyboard);
            assert_eq!(p.batches, vec![Batch::Chord(keys)]);
            assert!(plan_with_policy(&Intent::KeyChord, Some(chord), false).is_err());
        }
        for bad in [
            "Ctrl+Alt+Delete",
            "Win+Win+E",
            "Win+Win",
            "Ctrl+Ctrl+S",
            "Win++E",
        ] {
            assert!(plan_with_policy(&Intent::KeyChord, Some(bad), true).is_err());
        }
    }
    #[test]
    fn physical_virtual_desktop_mapping_preserves_negative_origins() {
        assert_eq!(absolute_axis(-3600.0, -3600, 7200).unwrap(), 0);
        assert_eq!(absolute_axis(3599.0, -3600, 7200).unwrap(), 65535);
        assert!(absolute_axis(3600.0, -3600, 7200).is_err());
        assert!(absolute_axis(f64::NAN, 0, 3600).is_err());
    }
    struct Fake {
        sent: usize,
        guards: usize,
        fail_after: Option<usize>,
        cancel_in_guard: Option<Operation>,
        fail_send: bool,
        cancel_after_send: Option<Operation>,
    }
    impl Sink for Fake {
        fn guard(&mut self) -> Result<()> {
            self.guards += 1;
            if let Some(op) = self.cancel_in_guard.take() {
                op.cancel();
            }
            if self.fail_after.is_some_and(|n| self.sent >= n) {
                return Err(fault("focus_changed", "No input."));
            }
            Ok(())
        }
        fn send(&mut self, _: &Batch) -> Result<()> {
            self.sent += 1;
            if let Some(op) = &self.cancel_after_send {
                op.cancel();
            }
            if self.fail_send {
                Err(fault("input_partial_unconfirmed", "Do not replay."))
            } else {
                Ok(())
            }
        }
    }
    fn fake() -> Fake {
        Fake {
            sent: 0,
            guards: 0,
            fail_after: None,
            cancel_in_guard: None,
            fail_send: false,
            cancel_after_send: None,
        }
    }
    #[test]
    fn wrong_focus_and_focus_loss_never_dispatch_following_batches() {
        let p = plan(&Intent::Text, Some("abc")).unwrap();
        for (fail_after, count) in [(0, 0), (1, 1)] {
            let mut f = fake();
            f.fail_after = Some(fail_after);
            assert_eq!(execute(&p, &mut f).unwrap_err().code, "focus_changed");
            assert_eq!(f.sent, count);
        }
    }
    #[test]
    fn user_stop_during_guard_blocks_the_next_send_and_partial_is_not_retried() {
        let p = plan(&Intent::Text, Some("abc")).unwrap();
        let op = Registry::default().begin(None).unwrap();
        let mut f = fake();
        f.cancel_in_guard = Some(op.clone());
        operation::run(&op, || {
            assert_eq!(execute(&p, &mut f).unwrap_err().code, "operation_cancelled")
        });
        assert_eq!(f.sent, 0);
        let mut f = fake();
        f.fail_send = true;
        assert_eq!(
            execute(&p, &mut f).unwrap_err().code,
            "input_partial_unconfirmed"
        );
        assert_eq!(f.sent, 1);
    }
    #[test]
    fn stop_after_first_character_and_expired_lease_do_not_send_the_remainder() {
        let p = plan(&Intent::Text, Some("abc")).unwrap();
        let op = Registry::default().begin(None).unwrap();
        let mut f = fake();
        f.cancel_after_send = Some(op.clone());
        operation::run(&op, || {
            assert_eq!(execute(&p, &mut f).unwrap_err().code, "operation_cancelled")
        });
        assert_eq!(f.sent, 1);
        let mut f = fake();
        operation::with_dispatch_deadline(std::time::Instant::now(), || {
            assert_eq!(
                execute(&p, &mut f).unwrap_err().code,
                "lease_expired_during_dispatch"
            )
        });
        assert_eq!(f.sent, 0);
    }

    #[derive(Clone, Copy)]
    enum DuringDocument {
        None,
        Focus,
        Geometry,
        Epoch,
        Layout,
        Session,
        Hit,
        User,
        GrantExpiry,
    }
    struct GuardFixture {
        change: DuringDocument,
        window: u32,
        title: &'static str,
        value: &'static str,
        focus: &'static str,
        rectangle: [i32; 4],
        native_error: Option<&'static str>,
        cancel_in_document: Option<Operation>,
        wait_in_document_until: Option<std::time::Instant>,
        trace: Vec<&'static str>,
        sent: usize,
    }
    impl GuardFixture {
        fn new(change: DuringDocument) -> Self {
            Self {
                change,
                window: 7,
                title: "same title",
                value: "same value",
                focus: "exact-A",
                rectangle: [0, 0, 300, 200],
                native_error: None,
                cancel_in_document: None,
                wait_in_document_until: None,
                trace: vec![],
                sent: 0,
            }
        }
    }
    impl GuardPhases for GuardFixture {
        fn document_precondition(&mut self) -> Result<()> {
            if self.sent != 0 {
                self.trace.push("document_skipped");
                return Ok(());
            }
            self.trace.push("document");
            // Model a native provider call that returns the same document hash,
            // but changes other native state before returning to the guard.
            match self.change {
                DuringDocument::None => (),
                DuringDocument::Focus => self.focus = "exact-B",
                DuringDocument::Geometry => self.rectangle[0] = 400,
                DuringDocument::Epoch => self.native_error = Some("input_focus_or_window_changed"),
                DuringDocument::Layout => self.native_error = Some("input_display_changed"),
                DuringDocument::Session => self.native_error = Some("session_mismatch"),
                DuringDocument::Hit => self.native_error = Some("input_target_occluded"),
                DuringDocument::User => self.native_error = Some("user_input_active"),
                DuringDocument::GrantExpiry => self.native_error = Some("input_grant_expired"),
            }
            if let Some(op) = &self.cancel_in_document {
                op.cancel();
            }
            if let Some(deadline) = self.wait_in_document_until {
                std::thread::sleep(
                    deadline.saturating_duration_since(std::time::Instant::now())
                        + std::time::Duration::from_millis(1),
                );
            }
            assert_eq!(
                (self.window, self.title, self.value),
                (7, "same title", "same value")
            );
            Ok(())
        }
        fn exact_input_scope(&mut self) -> Result<()> {
            self.trace.push("exact_scope");
            if self.focus != "exact-A" {
                return Err(fault("keyboard_focus_changed", "No input."));
            }
            Ok(())
        }
        fn final_native_state(&mut self) -> Result<()> {
            self.trace.push("native_state");
            if self.rectangle != [0, 0, 300, 200] {
                return Err(fault("input_focus_or_window_changed", "No input."));
            }
            if let Some(code) = self.native_error {
                return Err(fault(code, "No input."));
            }
            Ok(())
        }
    }
    impl Sink for GuardFixture {
        fn guard(&mut self) -> Result<()> {
            // Exactly the helper used by NativeSink, not a copied test order.
            guard_before_batch(self)
        }
        fn send(&mut self, _: &Batch) -> Result<()> {
            self.trace.push("send");
            self.sent += 1;
            self.value = "changed by our input";
            Ok(())
        }
    }
    #[test]
    fn document_return_is_followed_by_exact_scope_and_native_guards_before_each_batch() {
        let p = plan(&Intent::Text, Some("ab")).unwrap();
        let mut f = GuardFixture::new(DuringDocument::None);
        execute(&p, &mut f).unwrap();
        assert_eq!(f.sent, 2);
        assert_eq!(
            f.trace,
            [
                "document",
                "exact_scope",
                "native_state",
                "send",
                "document_skipped",
                "exact_scope",
                "native_state",
                "send",
            ]
        );
    }
    #[test]
    fn same_hwnd_title_and_value_do_not_hide_focus_or_geometry_change_during_document_read() {
        let p = plan(&Intent::Text, Some("ab")).unwrap();
        for (change, code, expected_trace) in [
            (
                DuringDocument::Focus,
                "keyboard_focus_changed",
                vec!["document", "exact_scope"],
            ),
            (
                DuringDocument::Geometry,
                "input_focus_or_window_changed",
                vec!["document", "exact_scope", "native_state"],
            ),
        ] {
            let mut f = GuardFixture::new(change);
            assert_eq!(execute(&p, &mut f).unwrap_err().code, code);
            assert_eq!(f.sent, 0);
            assert_eq!(f.trace, expected_trace);
        }
    }
    #[test]
    fn final_guards_reject_epoch_layout_session_hit_user_and_grant_changes_during_document_read() {
        let p = plan(&Intent::Click, None).unwrap();
        for (change, code) in [
            (DuringDocument::Epoch, "input_focus_or_window_changed"),
            (DuringDocument::Layout, "input_display_changed"),
            (DuringDocument::Session, "session_mismatch"),
            (DuringDocument::Hit, "input_target_occluded"),
            (DuringDocument::User, "user_input_active"),
            (DuringDocument::GrantExpiry, "input_grant_expired"),
        ] {
            let mut f = GuardFixture::new(change);
            assert_eq!(execute(&p, &mut f).unwrap_err().code, code);
            assert_eq!(f.sent, 0);
            assert_eq!(f.trace, ["document", "exact_scope", "native_state"]);
        }
    }
    #[test]
    fn cancellation_or_lease_expiry_during_document_read_blocks_all_following_guard_phases_and_send()
     {
        let p = plan(&Intent::Text, Some("x")).unwrap();
        let op = Registry::default().begin(None).unwrap();
        let mut f = GuardFixture::new(DuringDocument::None);
        f.cancel_in_document = Some(op.clone());
        operation::run(&op, || {
            assert_eq!(execute(&p, &mut f).unwrap_err().code, "operation_cancelled");
        });
        assert_eq!(f.trace, ["document"]);
        assert_eq!(f.sent, 0);

        let deadline = std::time::Instant::now() + std::time::Duration::from_millis(100);
        let mut f = GuardFixture::new(DuringDocument::None);
        f.wait_in_document_until = Some(deadline);
        operation::with_dispatch_deadline(deadline, || {
            assert_eq!(
                execute(&p, &mut f).unwrap_err().code,
                "lease_expired_during_dispatch"
            );
        });
        assert_eq!(f.trace, ["document"]);
        assert_eq!(f.sent, 0);
    }
}
