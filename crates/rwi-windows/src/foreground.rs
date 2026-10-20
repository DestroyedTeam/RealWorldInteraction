//! Input foreground is either the selected window or its proven native owner.
//! This is grounding, not application authorization or permission to refocus.
use crate::{NativeWindowKey, WindowRecord};

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub(crate) struct Anchor {
    pub key: NativeWindowKey,
    pub native_epoch: String,
}
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub(crate) struct State {
    pub window: String,
    pub anchor: Option<Anchor>,
}
impl Anchor {
    pub(crate) fn attest(
        selected: &WindowRecord,
        foreground: &WindowRecord,
        allow_owned: bool,
    ) -> Option<Self> {
        let key = foreground.key()?;
        let selected_key = selected.key()?;
        let epoch = foreground.native_epoch.as_ref()?;
        if !foreground.foreground
            || !foreground.visible
            || foreground.minimized
            || foreground.cloaked != Some(false)
            || selected.native_epoch.is_none()
        {
            return None;
        }
        let exact = selected_key == key && selected.native_epoch.as_ref() == Some(epoch);
        let owned = allow_owned
            && selected_key != key
            && selected.owner_chain_complete
            && selected.owner_chain.contains(&key);
        (exact || owned).then(|| Self {
            key,
            native_epoch: epoch.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn window(id: &str) -> WindowRecord {
        WindowRecord {
            window: id.into(),
            pid: 42,
            process_created: Some(10),
            native_epoch: Some(format!("epoch-{id}")),
            title: "same".into(),
            title_truncated: false,
            class: "generic".into(),
            rectangle: None,
            dwm_visible_frame: None,
            visible: true,
            enabled: true,
            minimized: false,
            cloaked: Some(false),
            foreground: true,
            window_dpi: Some(96),
            window_dpi_awareness: 2,
            owner_chain: vec![],
            owner_chain_complete: true,
            parent: None,
            z_index: 0,
        }
    }
    #[test]
    fn foreground_root_and_modal_dialog_do_not_need_an_owner_exception() {
        let root = window("root");
        let mut dialog = window("dialog");
        dialog.owner_chain.push(root.key().unwrap());
        assert!(Anchor::attest(&root, &root, false).is_some());
        assert!(Anchor::attest(&dialog, &dialog, false).is_some());
        assert!(Anchor::attest(&root, &dialog, true).is_none());
    }
    #[test]
    fn nonactivating_popup_uses_exact_foreground_owner_only_under_desktop_policy() {
        let root = window("root");
        let mut popup = window("popup");
        popup.foreground = false;
        popup.owner_chain.push(root.key().unwrap());
        let anchor = Anchor::attest(&popup, &root, true).unwrap();
        assert_eq!(anchor.key, root.key().unwrap());
        assert!(Anchor::attest(&popup, &root, false).is_none());
        let mut submenu = window("submenu");
        submenu.foreground = false;
        submenu.owner_chain = vec![popup.key().unwrap(), root.key().unwrap()];
        assert_eq!(Anchor::attest(&submenu, &root, true), Some(anchor));
    }
    #[test]
    fn same_pid_title_or_sibling_is_not_foreground_ownership() {
        let root = window("root");
        let mut a = window("a");
        a.owner_chain.push(root.key().unwrap());
        let mut b = window("b");
        b.owner_chain.push(root.key().unwrap());
        assert!(Anchor::attest(&a, &b, true).is_none());
        a.owner_chain.clear();
        assert!(Anchor::attest(&a, &root, true).is_none());
    }
    #[test]
    fn owner_reuse_incomplete_chain_or_lost_foreground_stops_input() {
        let mut root = window("root");
        let mut popup = window("popup");
        popup.owner_chain.push(root.key().unwrap());
        root.process_created = Some(11);
        assert!(Anchor::attest(&popup, &root, true).is_none());
        root.process_created = Some(10);
        popup.owner_chain_complete = false;
        assert!(Anchor::attest(&popup, &root, true).is_none());
        popup.owner_chain_complete = true;
        root.foreground = false;
        assert!(Anchor::attest(&popup, &root, true).is_none());
        root.foreground = true;
        root.cloaked = None;
        assert!(Anchor::attest(&popup, &root, true).is_none());
    }
    #[test]
    fn fresh_foreground_epoch_is_part_of_the_binding_even_when_hwnd_is_same() {
        let mut root = window("root");
        let mut popup = window("popup");
        popup.owner_chain.push(root.key().unwrap());
        let old = Anchor::attest(&popup, &root, true);
        root.native_epoch = Some("new-epoch".into());
        assert_ne!(old, Anchor::attest(&popup, &root, true));
        root.native_epoch = None;
        assert!(Anchor::attest(&popup, &root, true).is_none());
    }
    #[test]
    fn unrelated_foreground_changes_are_visible_even_without_an_input_anchor() {
        let before = State {
            window: "a".into(),
            anchor: None,
        };
        let after = State {
            window: "b".into(),
            anchor: None,
        };
        assert_ne!(before, after);
    }
}
