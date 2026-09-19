//! Stateful fake AX reads change when fake posts mutate value/focus/title.
use super::*;
use std::{cell::RefCell, rc::Rc, sync::atomic::Ordering};

#[derive(Clone)]
struct Fake {
    identity: Identity,
    process: String,
    scope: Scope,
    window: u64,
    hit: u64,
    hit_id: String,
    focused: Option<u64>,
    window_rect: Rect,
    hit_rect: Rect,
    point: [f64; 2],
    layout: String,
    permissions: bool,
    modal: Option<bool>,
    occluded: bool,
    enabled: Option<bool>,
    writable: Option<bool>,
    class: TextClass,
    mapping_ready: bool,
    value: String,
    title: String,
    posts: Vec<bool>,
    first_reads: usize,
    continuation_reads: usize,
}
impl Fake {
    fn new() -> Self {
        Self {
            identity: Identity {
                host: "native-fake-host".into(),
                guest: None,
                session: "uid:501:security:1:boot:1".into(),
                seat: "logical-console".into(),
                desktop: "quartz-console".into(),
            },
            process: "20:100:1".into(),
            scope: Scope {
                window: "cgwindow:10".into(),
                pid: 20,
                element: Some("original-element".into()),
            },
            window: 10,
            hit: 20,
            hit_id: "original-element".into(),
            focused: Some(20),
            window_rect: Rect {
                x: 0.,
                y: 0.,
                w: 200.,
                h: 100.,
            },
            hit_rect: Rect {
                x: 0.,
                y: 0.,
                w: 100.,
                h: 20.,
            },
            point: [50., 10.],
            layout: "original-layout".into(),
            permissions: true,
            modal: Some(false),
            occluded: false,
            enabled: Some(true),
            writable: Some(true),
            class: TextClass::Plain,
            mapping_ready: true,
            value: String::new(),
            title: "Document".into(),
            posts: vec![],
            first_reads: 0,
            continuation_reads: 0,
        }
    }
    fn scene(&self) -> Scene {
        Scene {
            identity: self.identity.clone(),
            assurance: "native-attested".into(),
            binding: Some(Binding {
                scope: Scope {
                    element: None,
                    ..self.scope.clone()
                },
                focus_window: self.scope.window.clone(),
                layout: self.layout.clone(),
                native_epoch: digest(format!("{}:{:?}", self.title, self.focused).as_bytes()),
                content: digest(format!("{}:{}", self.value, self.title).as_bytes()),
                mutation_context: None,
            }),
            mutation_ready: true,
            blocked: Some(false),
            image_size: [0, 0],
            pixel_to_desktop: None,
            elements: vec![Element {
                enabled: None,
                actions: vec![],
                role: None,
                id: self.hit_id.clone(),
                name: "Input".into(),
                point: self.point,
                visible: Some(true),
                occluded: Some(false),
                state_revision: Some(digest(self.value.as_bytes())),
                selected: None,
                value: None,
                value_truncated: false,
                description: None,
            }],
            captured_unix_ms: 1,
            atomicity: "stateful fake".into(),
            image_normalization: "no image".into(),
            png: vec![],
        }
    }
    fn proposal(&self) -> (Identity, Binding, Resolved) {
        let scene = self.scene();
        (
            self.identity.clone(),
            scene.binding.clone().unwrap(),
            Resolved {
                reference: "original-ref".into(),
                target: "target".into(),
                frame: "original-frame".into(),
                point: self.point,
                placement_only: false,
                scope: self.scope.clone(),
                element_revision: self
                    .scope
                    .element
                    .as_ref()
                    .map(|_| scene.elements[0].revision()),
                source_region: None,
            },
        )
    }
    fn read(
        &mut self,
        first: bool,
        identity: &Identity,
        binding: &Binding,
        target: &Resolved,
    ) -> Result<Facts<u64>> {
        if first {
            self.first_reads += 1;
            validate_batch_scene(identity, binding, target, &self.scene())?;
        } else {
            self.continuation_reads += 1;
        }
        Ok(Facts {
            identity: self.identity.clone(),
            process: self.process.clone(),
            scope: self.scope.clone(),
            window: self.window,
            window_rect: self.window_rect,
            hit: self.hit,
            hit_rect: self.hit_rect,
            hit_id: self.hit_id.clone(),
            focused: self.focused,
            point: self.point,
            layout: self.layout.clone(),
            mapping_ready: self.mapping_ready,
            permissions: self.permissions,
            modal: self.modal,
            occluded: self.occluded,
            enabled: self.enabled,
            writable: self.writable,
            class: self.class,
        })
    }
    fn post(&mut self, event: &Event) {
        self.posts.push(event.down);
        if event.down {
            self.value
                .push_str(&String::from_utf16(&event.units).unwrap());
            self.title = "Document — Edited".into();
            self.focused = Some(self.hit);
        }
    }
    fn mutate(&mut self, case: usize) {
        match case {
            0 => self.identity.session = "different-native-login".into(),
            1 => self.process = "20:101:1".into(),
            2 => self.scope.window = "cgwindow:11".into(),
            3 => self.window = 999,
            4 => self.hit = 999,
            5 => self.hit_id = "replacement-element".into(),
            6 => self.window_rect.x += 1.,
            7 => self.hit_rect.w += 1.,
            8 => self.layout = "changed-display-layout".into(),
            9 => self.point[0] += 1.,
            10 => self.permissions = false,
            11 => self.class = TextClass::Secure,
            12 => self.class = TextClass::Unknown,
            13 => self.writable = Some(false),
            14 => self.writable = None,
            15 => self.modal = Some(true),
            16 => self.modal = None,
            17 => self.enabled = Some(false),
            18 => self.enabled = None,
            19 => self.occluded = true,
            20 => self.focused = Some(999),
            21 => self.mapping_ready = false,
            _ => self.class = TextClass::NotText,
        }
    }
}
#[derive(Clone)]
struct Event {
    down: bool,
    units: Vec<u16>,
}
fn events(units: Vec<u16>) -> (Event, Event) {
    (
        Event { down: true, units },
        Event {
            down: false,
            units: vec![],
        },
    )
}
type Proposal = (Identity, Binding, Resolved);
fn pair(
    sequence: &mut Sequence<u64>,
    state: &Rc<RefCell<Fake>>,
    proposal: &Proposal,
    counter: &AtomicU64,
    units: Vec<u16>,
) -> Result<()> {
    sequence.pair(
        &proposal.1,
        &proposal.2,
        counter,
        events(units),
        |first| {
            state
                .borrow_mut()
                .read(first, &proposal.0, &proposal.1, &proposal.2)
        },
        |event| state.borrow_mut().post(event),
    )
}

#[test]
fn text_over_twenty_utf16_units_continues_after_its_own_ax_value_and_title_changes() {
    let state = Rc::new(RefCell::new(Fake::new()));
    let proposal = state.borrow().proposal();
    let counter = AtomicU64::new(0);
    let mut sequence = Sequence::new(&Intent::Text);
    let text = "中文🙂 repeated text beyond twenty UTF16 units — second and third chunks 🙂";
    let chunks = utf16_chunks(text, 20);
    assert!(chunks.len() > 2);
    for chunk in &chunks {
        pair(&mut sequence, &state, &proposal, &counter, chunk.clone()).unwrap();
    }
    let fake = state.borrow();
    assert_eq!(fake.value, text);
    assert_eq!(counter.load(Ordering::Acquire), 2 * chunks.len() as u64);
    assert_eq!(fake.posts, [true, false].repeat(chunks.len()));
    assert_eq!(
        (fake.first_reads, fake.continuation_reads),
        (1, chunks.len() - 1)
    );
    assert_ne!(
        fake.scene().elements[0].revision(),
        proposal.2.element_revision.as_ref().unwrap().as_str()
    );
    assert_ne!(
        fake.scene().binding.unwrap().native_epoch,
        proposal.1.native_epoch
    );
}
#[test]
fn double_click_allows_only_its_original_focus_to_original_native_hit_transition() {
    for pixel in [false, true] {
        let mut fake = Fake::new();
        fake.focused = Some(30);
        fake.class = TextClass::NotText;
        if pixel {
            fake.scope.element = None;
        }
        let proposal = fake.proposal();
        let state = Rc::new(RefCell::new(fake));
        let counter = AtomicU64::new(0);
        let mut sequence = Sequence::new(&Intent::DoubleClick);
        pair(&mut sequence, &state, &proposal, &counter, vec![]).unwrap();
        assert_eq!(state.borrow().focused, Some(20));
        assert_ne!(
            state.borrow().scene().binding.unwrap().native_epoch,
            proposal.1.native_epoch
        );
        pair(&mut sequence, &state, &proposal, &counter, vec![]).unwrap();
        assert_eq!(state.borrow().posts, [true, false, true, false]);
        assert_eq!(counter.load(Ordering::Acquire), 4);
    }
}
#[test]
fn changed_initial_revision_is_still_zero_posts_and_creates_no_continuation_authority() {
    let mut fake = Fake::new();
    let proposal = fake.proposal();
    fake.value = "external edit before first post".into();
    let state = Rc::new(RefCell::new(fake));
    let counter = AtomicU64::new(0);
    let mut sequence = Sequence::new(&Intent::Text);
    assert_eq!(
        pair(&mut sequence, &state, &proposal, &counter, vec![65])
            .unwrap_err()
            .code,
        "stale_reference"
    );
    assert_eq!(counter.load(Ordering::Acquire), 0);
    assert!(sequence.anchor.is_none());
}
#[test]
fn unknown_enabled_is_allowed_while_explicit_disabled_still_stops() {
    let state = Rc::new(RefCell::new(Fake::new()));
    let proposal = state.borrow().proposal();
    let counter = AtomicU64::new(0);
    let mut sequence = Sequence::new(&Intent::Text);
    pair(&mut sequence, &state, &proposal, &counter, vec![65]).unwrap();
    state.borrow_mut().mutate(18);
    pair(&mut sequence, &state, &proposal, &counter, vec![66]).unwrap();
    assert_eq!(state.borrow().value, "AB");
    assert_eq!(state.borrow().posts, [true, false, true, false]);
}
#[test]
fn each_native_boundary_change_stops_text_after_exactly_the_already_returned_pair() {
    for case in (0..23).filter(|case| *case != 18) {
        let state = Rc::new(RefCell::new(Fake::new()));
        let proposal = state.borrow().proposal();
        let counter = AtomicU64::new(0);
        let mut sequence = Sequence::new(&Intent::Text);
        pair(&mut sequence, &state, &proposal, &counter, vec![65]).unwrap();
        state.borrow_mut().mutate(case);
        assert!(
            pair(&mut sequence, &state, &proposal, &counter, vec![66]).is_err(),
            "case {case}"
        );
        assert_eq!(state.borrow().value, "A", "case {case}");
        assert_eq!(state.borrow().posts, [true, false]);
        assert_eq!(counter.load(Ordering::Acquire), 2);
    }
}
#[test]
fn pixel_double_click_cannot_switch_ax_objects_at_the_same_pid_and_point() {
    for case in [3, 4, 5, 8, 10, 11, 15, 17, 19, 20] {
        let mut fake = Fake::new();
        fake.focused = Some(30);
        fake.scope.element = None;
        let proposal = fake.proposal();
        let state = Rc::new(RefCell::new(fake));
        let counter = AtomicU64::new(0);
        let mut sequence = Sequence::new(&Intent::DoubleClick);
        pair(&mut sequence, &state, &proposal, &counter, vec![]).unwrap();
        state.borrow_mut().mutate(case);
        assert!(
            pair(&mut sequence, &state, &proposal, &counter, vec![]).is_err(),
            "case {case}"
        );
        assert_eq!(counter.load(Ordering::Acquire), 2);
        assert_eq!(state.borrow().posts, [true, false]);
    }
}
#[test]
fn cancellation_after_down_preserves_up_and_prevents_any_remaining_pair() {
    let state = Rc::new(RefCell::new(Fake::new()));
    let proposal = state.borrow().proposal();
    let counter = AtomicU64::new(0);
    let mut sequence = Sequence::new(&Intent::Text);
    let op = operation::Registry::default().begin(None).unwrap();
    operation::run(&op, || {
        sequence
            .pair(
                &proposal.1,
                &proposal.2,
                &counter,
                events(vec![65]),
                |first| {
                    state
                        .borrow_mut()
                        .read(first, &proposal.0, &proposal.1, &proposal.2)
                },
                |event| {
                    state.borrow_mut().post(event);
                    if event.down {
                        op.cancel();
                    }
                },
            )
            .unwrap();
        assert!(pair(&mut sequence, &state, &proposal, &counter, vec![66]).is_err());
    });
    assert_eq!(counter.load(Ordering::Acquire), 2);
    assert_eq!(state.borrow().posts, [true, false]);
    op.finish();
    assert!(op.status().resources_released);
}
#[test]
fn lease_expiration_stops_continuation_and_a_new_invocation_does_not_inherit_anchor() {
    let state = Rc::new(RefCell::new(Fake::new()));
    let proposal = state.borrow().proposal();
    let counter = AtomicU64::new(0);
    let mut sequence = Sequence::new(&Intent::Text);
    pair(&mut sequence, &state, &proposal, &counter, vec![65]).unwrap();
    operation::with_dispatch_deadline(Instant::now() - Duration::from_millis(1), || {
        assert!(pair(&mut sequence, &state, &proposal, &counter, vec![66]).is_err())
    });
    let mut another = Sequence::new(&Intent::Text);
    assert_eq!(
        pair(&mut another, &state, &proposal, &counter, vec![66])
            .unwrap_err()
            .code,
        "stale_reference"
    );
    assert!(another.anchor.is_none());
    assert_eq!(counter.load(Ordering::Acquire), 2);
}
