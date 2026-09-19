//! Correlate eligible roots while retaining the complete native inventory.
use super::*;
// Eligibility comes from native visibility/owner facts, never titles or the
// launcher PID. All windows remain in the raw before/after inventories.
pub(super) fn classify(r: &mut LaunchReceipt, baseline: usize) {
    let before = &r.before.as_ref().expect("before inventory").windows;
    let after = r
        .after
        .as_ref()
        .expect("after inventory")
        .windows
        .iter()
        .filter(|window| window.interactive_root())
        .collect::<Vec<_>>();
    if before.iter().any(|w| {
        r.native_events
            .events
            .iter()
            .skip(baseline)
            .any(|e| e.window == w.key.window && e.event == "destroy")
            && after.iter().any(|a| a.key == w.key)
    }) {
        r.state = LaunchState::Ambiguous;
        r.candidates = after.iter().map(|w| w.key.clone()).collect();
        r.correlation="Destroyed HWND reappears with the same process/class-derived epoch; there is no persistent native HWND generation counter. Never treat this as authorized existing-window reuse.".into();
        return;
    }
    let new = after
        .iter()
        .filter(|w| !before.iter().any(|b| b.key == w.key))
        .map(|w| w.key.clone())
        .collect::<Vec<_>>();
    let reused = after
        .iter()
        .filter(|w| {
            before
                .iter()
                .any(|b| b.key == w.key && (b.title != w.title || b.foreground != w.foreground))
                || r.native_events
                    .events
                    .iter()
                    .skip(baseline)
                    .any(|e| e.window == w.key.window && e.event != "destroy")
        })
        .map(|w| w.key.clone())
        .collect::<Vec<_>>();
    (r.state,r.candidates,r.correlation) = match new.len() {
        1 => (LaunchState::NewWindowObserved,new,"Exactly one new HWND/PID/birth/epoch for an allowed image in the bounded interval; temporal native inventory/event evidence, not proof of launch causality. Broker PID may differ; no input authority.".into()),
        2.. => (LaunchState::Ambiguous,new,"Multiple new allowed-image windows. Do not choose by title, returned PID or foreground alone.".into()),
        _ => match reused.len() {
            1 => (LaunchState::ReusedExistingWindow,reused,"One explicitly authorized existing window changed or emitted native events. A new tab in the same HWND is reuse, not a new window; tab/content semantics remain unverified.".into()),
            2.. => (LaunchState::Ambiguous,reused,"Multiple existing windows show reuse evidence; no unique attribution.".into()),
            _ => (LaunchState::LaunchedNoWindow,vec![],"CreateProcessW accepted; no new window or observable reuse during the bounded interval. Not proof the application will never create a window.".into()),
        }
    };
}

#[cfg(test)]
mod tests {
    use super::super::test_support::window;
    use super::*;
    use serde_json::json;

    fn receipt(before: Vec<WindowEvidence>, after: Vec<WindowEvidence>) -> LaunchReceipt {
        let inventory = |windows| {
            json!({"identity":{"host":"fixture","guest":null,
            "session":"fixture","seat":"fixture","desktop":"fixture"},
            "windows":windows,"complete":true,"native_windows_enumerated":16,
            "captured_unix_ms":1,"notes":[]})
        };
        serde_json::from_value(json!({"request_id":"fixture","authorization_id":"fixture",
            "state":"not_dispatched","before":inventory(before),"after":inventory(after),
            "native_events":{"events":[],"complete":true,"hooks_released":true},
            "native_launch":null,"candidates":[],"correlation":"","error":null,"replay":"none",
            "resources_released":true,"resource_release_uncertain":false,
            "goal_verified":false,"input_authority_granted":false}))
        .unwrap()
    }

    #[test]
    fn hidden_ime_creation_does_not_displace_one_reused_notepad_root() {
        let before = window(1);
        let mut root = before.clone();
        root.title = "allocated.txt - Notepad".into();
        let mut ime = window(2);
        ime.title = "Default IME".into();
        ime.candidate_facts.as_mut().unwrap().visible = false;
        let mut other_ime = ime.clone();
        other_ime.key = window(3).key;
        let mut r = receipt(vec![before], vec![root.clone(), ime, other_ime]);
        classify(&mut r, 0);
        assert_eq!(r.state, LaunchState::ReusedExistingWindow);
        assert_eq!(r.candidates, vec![root.key]);
        assert_eq!(r.after.as_ref().unwrap().windows.len(), 3);
        assert!(!r.input_authority_granted);
    }

    #[test]
    fn two_real_roots_still_ambiguous_regardless_of_titles() {
        let mut r = receipt(vec![], vec![window(1), window(2)]);
        classify(&mut r, 0);
        assert_eq!(r.state, LaunchState::Ambiguous);
        assert_eq!(r.candidates.len(), 2);
    }

    #[test]
    fn owned_cloaked_incomplete_or_missing_native_facts_never_become_roots() {
        for variant in 0..5 {
            let mut w = window(1);
            match variant {
                0 => w.candidate_facts.as_mut().unwrap().owned = true,
                1 => w.candidate_facts.as_mut().unwrap().cloaked = Some(true),
                2 => w.candidate_facts.as_mut().unwrap().owner_chain_complete = false,
                3 => w.candidate_facts.as_mut().unwrap().cloaked = None,
                _ => w.candidate_facts = None,
            }
            let mut r = receipt(vec![], vec![w]);
            classify(&mut r, 0);
            assert_eq!(r.state, LaunchState::LaunchedNoWindow);
            assert!(r.candidates.is_empty());
        }
    }
}
