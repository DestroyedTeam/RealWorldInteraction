//! Deterministic synthetic app. Compiled only by tests or explicit `fixture` feature.
use crate::*;
use std::sync::{Arc, Mutex};
#[derive(Clone)]
pub struct Fixture(pub Arc<Mutex<App>>);
pub struct App {
    pub identity: Identity,
    pub epoch: u64,
    pub pixel_epoch: u64,
    pub transform: Option<[f64; 6]>,
    pub dialog: bool,
    pub value: String,
    pub option: bool,
    pub calls: u64,
    pub ack_without_effect: bool,
    pub disconnect: bool,
    pub blocked: Option<bool>,
    pub ready: bool,
    pub png_bytes: usize,
    pub wrong_oracle: Option<Scope>,
    pub assurance: String,
}
impl Default for Fixture {
    fn default() -> Self {
        Self(Arc::new(Mutex::new(App {
            identity: Identity {
                host: "fixture-host".into(),
                guest: None,
                session: "session-1".into(),
                seat: "seat-0".into(),
                desktop: "desktop-1".into(),
            },
            epoch: 0,
            pixel_epoch: 0,
            transform: Some([0.5, 0.0, 0.0, 0.5, -1920.0, 0.0]),
            dialog: false,
            value: "test-option:off".into(),
            option: false,
            calls: 0,
            ack_without_effect: false,
            disconnect: false,
            blocked: Some(false),
            ready: true,
            png_bytes: 0,
            wrong_oracle: None,
            assurance: "synthetic-fixture".into(),
        })))
    }
}
impl App {
    fn binding(&self) -> Binding {
        Binding {
            scope: Scope {
                window: "fixture-window".into(),
                pid: 42,
                element: None,
            },
            focus_window: "fixture-window".into(),
            layout: format!("{:?}", self.transform),
            content: format!(
                "{}:{}:{}:{}",
                self.epoch, self.value, self.dialog, self.pixel_epoch
            ),
            native_epoch: format!("fixture-epoch-{}", self.epoch),
            mutation_context: Some(MutationContext {
                window_title: "Fixture App".into(),
                document_state: Some(digest(self.value.as_bytes())),
            }),
        }
    }
}
impl Backend for Fixture {
    fn identity(&self) -> Identity {
        self.0.lock().unwrap().identity.clone()
    }
    fn capabilities(&self) -> serde_json::Value {
        serde_json::json!({"adapter":"synthetic-fixture","native":false,"capture_visual":{"implemented":true,"native":false,"source":"fixed synthetic 1x1 grayscale-alpha PNG; not application pixels"}})
    }
    fn capture_visual(&mut self, options: VisualOptions) -> Result<VisualCapture> {
        options.validate()?;
        operation::checkpoint("fixture_visual")?;
        let a = self.0.lock().unwrap();
        Ok(VisualCapture {
            identity: a.identity.clone(),
            window: lifecycle::WindowIdentity {
                window: "fixture-window".into(),
                pid: 42,
                process_created: 1,
                native_epoch: format!("fixture-epoch-{}", a.epoch),
            },
            source_rect: VisualRect {
                x: -1920,
                y: 0,
                width: 1,
                height: 1,
            },
            image_size: [1, 1],
            visual_to_desktop: [1., 0., 0., 1., -1920., 0.],
            captured_unix_ms: lifecycle::now_ms(),
            assurance: "synthetic-fixture".into(),
            atomicity: "synthetic-locked; no native capture".into(),
            normalization: "fixed synthetic 1x1 grayscale-alpha PNG; no app pixels or authority"
                .into(),
            feedback: VisualFeedback {
                requested: options.feedback,
                suspension_ticket: None,
                exclusion: "fixture_has_no_presenter".into(),
                before_copy: serde_json::json!({"presenter":"not_implemented","active_suspensions":0}),
                after_copy: serde_json::json!({"presenter":"not_implemented","active_suspensions":0}),
                after_release: serde_json::json!({"presenter":"not_implemented","active_suspensions":0}),
            },
            details: serde_json::json!({"native":false}),
            png: vec![
                137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0,
                1, 8, 4, 0, 0, 0, 181, 28, 12, 2, 0, 0, 0, 11, 73, 68, 65, 84, 120, 218, 99, 252,
                255, 31, 0, 3, 3, 2, 0, 239, 162, 167, 91, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96,
                130,
            ],
        })
    }
    fn observe(&mut self) -> Result<Scene> {
        let a = self.0.lock().unwrap();
        Ok(Scene {
            identity: a.identity.clone(),
            assurance: a.assurance.clone(),
            binding: Some(a.binding()),
            mutation_ready: a.ready,
            blocked: a.blocked,
            image_size: [800, 600],
            pixel_to_desktop: a.transform,
            elements: ["Settings", "Toggle test option", "Apply", "Input"]
                .iter()
                .enumerate()
                .map(|(n, s)| Element {
                    role: None,
                    id: s.to_string(),
                    name: s.to_string(),
                    point: [100.0, 50.0 + 50.0 * n as f64],
                    visible: Some(true),
                    occluded: Some(false),
                    state_revision: Some(digest(if *s == "Input" {
                        a.value.as_bytes()
                    } else {
                        s.as_bytes()
                    })),
                })
                .collect(),
            captured_unix_ms: 0,
            atomicity: "synthetic-locked".into(),
            image_normalization: "synthetic bytes, not native capture".into(),
            png: vec![0; a.png_bytes],
        })
    }
    fn act(
        &mut self,
        expected: &Binding,
        target: &Resolved,
        intent: &Intent,
        value: Option<&str>,
    ) -> Result<Option<bool>> {
        let mut a = self.0.lock().unwrap();
        if a.binding().native_epoch != expected.native_epoch
            || a.binding().layout != expected.layout
            || (target.scope.element.is_none() && a.binding().content != expected.content)
        {
            return Err(fault("stale_reference", "Fixture changed before dispatch."));
        }
        a.calls += 1;
        if !a.ack_without_effect {
            match intent {
                Intent::Click | Intent::DoubleClick | Intent::ContextMenu => {
                    match target.scope.element.as_deref() {
                        Some("Settings") => a.dialog = true,
                        Some("Toggle test option") => {
                            a.option = !a.option;
                            a.value =
                                format!("test-option:{}", if a.option { "on" } else { "off" });
                        }
                        Some("Apply") => a.dialog = false,
                        _ => {}
                    }
                }
                Intent::Text => a.value = value.unwrap_or("").into(),
                Intent::Scroll | Intent::KeyChord | Intent::Drag(_) => {}
            }
            a.epoch += 1;
        }
        if a.disconnect {
            Err(fault(
                "transport_closed",
                "Effect may have happened; inspect, never replay.",
            ))
        } else {
            Ok(Some(true))
        }
    }
    fn inspect(&mut self, scope: &Scope, predicate: &Predicate, expected: &str) -> Result<Oracle> {
        let a = self.0.lock().unwrap();
        let matched = match predicate {
            Predicate::WindowTitle => expected == "Fixture App",
            Predicate::FocusedWindow => expected == "fixture-window",
            Predicate::ElementValue => {
                scope.element.as_deref() == Some("Input") && a.value == expected
            }
            Predicate::Modal => expected == if a.dialog { "Settings" } else { "none" },
        };
        Ok(Oracle {
            identity: a.identity.clone(),
            scope: a.wrong_oracle.clone().unwrap_or_else(|| scope.clone()),
            assurance: a.assurance.clone(),
            matched,
            source: "independent-synthetic-model".into(),
        })
    }
}
