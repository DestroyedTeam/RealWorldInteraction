//! Serializable Windows evidence and offline policy. A record is not attestation.
//! Controller host / guest claims deliberately do not occur in these native types.
use rcu_core::{Identity, Result, fault};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthenticationId {
    pub high: i32,
    pub low: u32,
}

/// Explicit deployment expectation. This enum never elevates a process or
/// substitutes for querying the real token. Both modes use the same fixed
/// installation, private state and durable HOLD policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrivilegeMode {
    Ordinary,
    Admin,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TokenPrivilegeFacts {
    pub elevated: bool,
    /// Native TOKEN_ELEVATION_TYPE: Default=1, Full=2, Limited=3.
    pub elevation_type: u32,
    pub integrity_rid: u32,
    pub ui_access: bool,
    pub app_container: bool,
}
impl TokenPrivilegeFacts {
    pub fn require_mode(&self, mode: PrivilegeMode) -> Result<()> {
        let coherent = match self.elevation_type {
            1 => true,
            2 => self.elevated,
            3 => !self.elevated,
            _ => false,
        };
        let mode_matches = match mode {
            PrivilegeMode::Ordinary => {
                !self.elevated && (0x2000..0x3000).contains(&self.integrity_rid)
            }
            PrivilegeMode::Admin => self.elevated && (0x3000..0x4000).contains(&self.integrity_rid),
        };
        if !coherent || !mode_matches || self.ui_access || self.app_container {
            return Err(fault(
                "agent_privilege_mode_mismatch",
                "Explicit ordinary/admin mode must match real elevation/type/integrity facts; SYSTEM, low integrity, UIAccess and AppContainer are not silently substituted or enabled.",
            ));
        }
        Ok(())
    }
    pub fn require_same(&self, current: &Self) -> Result<()> {
        if self != current {
            return Err(fault(
                "agent_token_privilege_changed",
                "Retained process token privilege facts changed; acquire a fresh explicitly authorized owner/session rather than continuing or clearing HOLD.",
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod privilege_tests {
    use super::*;
    #[test]
    fn explicit_ordinary_and_admin_match_measured_tokens_not_configuration_labels() {
        let ordinary = TokenPrivilegeFacts {
            elevated: false,
            elevation_type: 3,
            integrity_rid: 0x2000,
            ui_access: false,
            app_container: false,
        };
        let admin = TokenPrivilegeFacts {
            elevated: true,
            elevation_type: 2,
            integrity_rid: 0x3000,
            ui_access: false,
            app_container: false,
        };
        ordinary.require_mode(PrivilegeMode::Ordinary).unwrap();
        admin.require_mode(PrivilegeMode::Admin).unwrap();
        assert!(ordinary.require_mode(PrivilegeMode::Admin).is_err());
        assert!(admin.require_mode(PrivilegeMode::Ordinary).is_err());
        assert!(ordinary.require_same(&admin).is_err());
        let mut native_default = admin.clone();
        native_default.elevation_type = 1;
        native_default.require_mode(PrivilegeMode::Admin).unwrap(); // UAC policy can use Default.
        let mut invalid = vec![admin.clone(); 6];
        invalid[0].integrity_rid = 0x4000;
        invalid[1].integrity_rid = 0x1000;
        invalid[2].ui_access = true;
        invalid[3].app_container = true;
        invalid[4].elevation_type = 3;
        invalid[5].elevation_type = 99;
        for facts in invalid {
            assert!(facts.require_mode(PrivilegeMode::Admin).is_err());
        }
        assert!(serde_json::from_str::<PrivilegeMode>("\"auto\"").is_err());
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionBinding {
    pub native: Identity,
    pub user_sid: String,
    pub session_id: u32,
    pub authentication_id: AuthenticationId,
    pub logon_time_100ns: i64,
    pub input_desktop: String,
}

impl SessionBinding {
    pub fn validate(&self) -> Result<()> {
        validate_user_sid(&self.user_sid)?;
        let session = format!(
            "wts:{}:logon:{}:luid:{:x}:{:x}",
            self.session_id,
            self.logon_time_100ns,
            self.authentication_id.high,
            self.authentication_id.low
        );
        if self.session_id == 0
            || self.logon_time_100ns <= 0
            || self.authentication_id == (AuthenticationId { high: 0, low: 0 })
            || self.native.guest.is_some()
            || !self
                .native
                .host
                .strip_prefix("windows-machine:")
                .is_some_and(valid_sha256)
            || self.native.session != session
            || !self.native.seat.eq_ignore_ascii_case("WinSta0")
            || self.native.desktop.is_empty()
            || self.native.desktop.len() > 255
            || self.native.desktop.chars().any(char::is_control)
            || self.input_desktop != self.native.desktop
        {
            return Err(fault(
                "agent_session_binding_invalid",
                "Require separately observed native interactive SID, logon, station and input desktop; no controller/guest claim substitutes for these fields.",
            ));
        }
        Ok(())
    }

    pub fn require_same(&self, current: &Self) -> Result<()> {
        self.validate()?;
        current.validate()?;
        if self != current {
            return Err(fault(
                "agent_session_binding_changed",
                "Hold dispatch after any native SID/session/logon/desktop change.",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProcessBinding {
    pub pid: u32,
    pub creation_filetime_100ns: u64,
    pub user_sid: String,
    pub session_id: u32,
    pub authentication_id: AuthenticationId,
    pub image_path: String,
    pub image_sha256: String,
}

impl ProcessBinding {
    pub fn matches_session(&self, session: &SessionBinding) -> Result<()> {
        session.validate()?;
        validate_user_sid(&self.user_sid)?;
        validate_local_dos_path(&self.image_path)?;
        if self.pid == 0
            || self.creation_filetime_100ns == 0
            || self.user_sid != session.user_sid
            || self.session_id != session.session_id
            || self.authentication_id != session.authentication_id
            || !valid_sha256(&self.image_sha256)
        {
            return Err(fault(
                "agent_process_binding_invalid",
                "Require the retained process handle's SID/session/logon, creation time and pinned image digest.",
            ));
        }
        Ok(())
    }

    pub fn require_same(&self, current: &Self, session: &SessionBinding) -> Result<()> {
        self.matches_session(session)?;
        current.matches_session(session)?;
        if self != current {
            return Err(fault(
                "agent_process_binding_changed",
                "A matching PID or basename is insufficient; retain HOLD instead of reclaiming another process.",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OwnerProof {
    pub session: SessionBinding,
    pub process: ProcessBinding,
}

impl OwnerProof {
    pub fn validate(&self) -> Result<()> {
        self.process.matches_session(&self.session)
    }
}

pub fn valid_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// Query-only HWND/process/token facts. Serialized records are evidence, never
/// authority: the agent route constructs them from native handles on every use.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WindowUserBinding {
    pub window: rcu_core::lifecycle::WindowIdentity,
    pub thread_id: u32,
    pub user_sid: String,
    pub session_id: u32,
    pub authentication_id: AuthenticationId,
    pub input_desktop: String,
}
impl WindowUserBinding {
    pub fn validate(&self, expected: &SessionBinding) -> Result<()> {
        expected.validate()?;
        validate_user_sid(&self.user_sid)?;
        let hwnd = self.window.window.strip_prefix("hwnd:").unwrap_or("");
        if hwnd.is_empty()
            || hwnd.len() > 16
            || !hwnd.bytes().all(|v| v.is_ascii_hexdigit())
            || u64::from_str_radix(hwnd, 16).ok().is_none_or(|v| v == 0)
            || self.window.pid == 0
            || self.window.process_created == 0
            || !valid_sha256(&self.window.native_epoch)
            || self.thread_id == 0
            || self.user_sid != expected.user_sid
            || self.session_id != expected.session_id
            || self.authentication_id != expected.authentication_id
            || self.input_desktop != expected.input_desktop
        {
            return Err(fault(
                "agent_window_user_binding_changed",
                "Require the exact HWND/PID/birth/native epoch and native thread desktop, SID, session and authentication LUID; same PID/title is not authority.",
            ));
        }
        Ok(())
    }
}

/// Pure policy used only after fresh OS/UIA observations. The allocated root is
/// immutable across root -> independent owned HWND -> return-to-owner changes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SelectedWindowProof {
    pub session: SessionBinding,
    pub allocated_root: WindowUserBinding,
    pub selected: WindowUserBinding,
    pub owner_chain: Vec<WindowUserBinding>,
    pub owner_chain_complete: bool,
    pub foreground: rcu_core::lifecycle::WindowIdentity,
    pub root_uia_runtime_id: Vec<i32>,
    pub selected_uia_runtime_id: Vec<i32>,
    pub catalog_id: String,
}
impl SelectedWindowProof {
    pub fn require_scope(
        &self,
        expected_session: &SessionBinding,
        root: &rcu_core::lifecycle::WindowIdentity,
        root_runtime: &[i32],
    ) -> Result<()> {
        expected_session.require_same(&self.session)?;
        self.allocated_root.validate(expected_session)?;
        self.selected.validate(expected_session)?;
        for owner in &self.owner_chain {
            owner.validate(expected_session)?;
        }
        let mut keys = std::collections::BTreeSet::new();
        let chain_valid = self.owner_chain_complete
            && self.owner_chain.len() <= 16
            && self.owner_chain.iter().all(|owner| {
                owner.window != self.selected.window
                    && keys.insert((
                        owner.window.window.as_str(),
                        owner.window.pid,
                        owner.window.process_created,
                        owner.window.native_epoch.as_str(),
                    ))
            });
        let selected_is_root = self.selected.window == *root;
        let proven_owned = self
            .owner_chain
            .iter()
            .any(|owner| owner == &self.allocated_root);
        if self.allocated_root.window != *root
            || self.foreground != self.selected.window
            || self.catalog_id.is_empty()
            || self.catalog_id.len() > 128
            || self.root_uia_runtime_id.is_empty()
            || self.root_uia_runtime_id.len() > 64
            || self.root_uia_runtime_id != root_runtime
            || self.selected_uia_runtime_id.is_empty()
            || self.selected_uia_runtime_id.len() > 64
            || (selected_is_root && self.selected_uia_runtime_id != self.root_uia_runtime_id)
            || !chain_valid
            || (!selected_is_root && !proven_owned)
        {
            return Err(fault(
                "agent_selected_window_scope_unproven",
                "Require an unchanged allocated root/UIA identity, a freshly selected foreground HWND and its complete native owner chain; foreign, stale, reused and unknown scopes stay held.",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SelectedInputProof {
    pub native: SelectedWindowProof,
    pub binding: rcu_core::Binding,
    pub target: rcu_core::Scope,
    pub observed_unix_ms: u64,
}

#[cfg(test)]
mod selected_window_tests {
    use super::*;
    fn session() -> SessionBinding {
        SessionBinding {
            native: Identity {
                host: format!("windows-machine:{}", "1".repeat(64)),
                guest: None,
                session: "wts:1:logon:123:luid:0:456".into(),
                seat: "WinSta0".into(),
                desktop: "Default".into(),
            },
            user_sid: "S-1-5-21-1-2-3-1001".into(),
            session_id: 1,
            authentication_id: AuthenticationId {
                high: 0,
                low: 0x456,
            },
            logon_time_100ns: 123,
            input_desktop: "Default".into(),
        }
    }
    fn window(hwnd: &str, pid: u32, epoch: &str) -> WindowUserBinding {
        let s = session();
        WindowUserBinding {
            window: rcu_core::lifecycle::WindowIdentity {
                window: hwnd.into(),
                pid,
                process_created: 999,
                native_epoch: epoch.repeat(64),
            },
            thread_id: 42,
            user_sid: s.user_sid,
            session_id: s.session_id,
            authentication_id: s.authentication_id,
            input_desktop: s.input_desktop,
        }
    }
    fn popup() -> SelectedWindowProof {
        let root = window("hwnd:101", 101, "2");
        // Deliberately a separate HWND and process: PID equality is neither
        // necessary nor sufficient for the real native owner relation policy.
        let selected = window("hwnd:202", 202, "3");
        SelectedWindowProof {
            session: session(),
            allocated_root: root.clone(),
            foreground: selected.window.clone(),
            selected,
            owner_chain: vec![root],
            owner_chain_complete: true,
            root_uia_runtime_id: vec![42, 1],
            selected_uia_runtime_id: vec![42, 2],
            catalog_id: "fresh-native-catalog".into(),
        }
    }
    #[test]
    fn independent_owned_hwnd_and_explicit_return_keep_one_allocated_root() {
        let mut p = popup();
        let root = p.allocated_root.window.clone();
        p.require_scope(&session(), &root, &[42, 1]).unwrap();
        assert_ne!(p.selected.window.window, root.window);
        assert_ne!(p.selected.window.pid, root.pid);
        p.selected = p.allocated_root.clone();
        p.foreground = root.clone();
        p.owner_chain.clear();
        p.selected_uia_runtime_id = p.root_uia_runtime_id.clone();
        p.require_scope(&session(), &root, &[42, 1]).unwrap();
    }
    #[test]
    fn similar_same_pid_foreign_stale_reused_unknown_and_user_focus_are_denied() {
        let good = popup();
        let root = good.allocated_root.window.clone();
        let mut variants = vec![good.clone(); 15];
        variants[0].owner_chain.clear();
        variants[0].selected.window.pid = root.pid; // Same PID grants nothing.
        variants[0].foreground = variants[0].selected.window.clone();
        variants[1].owner_chain_complete = false;
        variants[2].owner_chain[0].window.process_created += 1;
        variants[3].owner_chain[0].window.native_epoch = "4".repeat(64);
        variants[4].selected.user_sid = "S-1-5-21-1-2-3-1002".into();
        variants[5].selected.session_id = 2;
        variants[6].selected.authentication_id.low += 1;
        variants[7].selected.input_desktop = "OtherDesktop".into();
        variants[8].foreground = root.clone(); // User returned elsewhere, not selected.
        variants[9].allocated_root.window.window = "hwnd:999".into();
        variants[10].root_uia_runtime_id = vec![42, 999];
        variants[11].selected_uia_runtime_id.clear();
        variants[12].catalog_id.clear();
        variants[13].owner_chain.push(good.selected.clone()); // Cycle.
        variants[14].selected.user_sid = "S-1-5-18".into();
        for (i, bad) in variants.iter().enumerate() {
            assert!(
                bad.require_scope(&session(), &root, &[42, 1]).is_err(),
                "negative {i}"
            );
        }
    }
    #[test]
    fn selected_proof_strict_schema_and_root_runtime_are_not_optional() {
        let p = popup();
        let root = p.allocated_root.window.clone();
        assert!(p.require_scope(&session(), &root, &[]).is_err());
        let mut json = serde_json::to_value(p).unwrap();
        json["model_claimed_filesystem_path"] = "C:\\not-authority.txt".into();
        assert!(serde_json::from_value::<SelectedWindowProof>(json).is_err());
    }
}
impl SelectedInputProof {
    pub fn require_target(
        &self,
        expected: &rcu_core::Binding,
        target: &rcu_core::Resolved,
    ) -> Result<()> {
        if self.binding.scope != expected.scope
            || self.binding.focus_window != expected.focus_window
            || self.binding.layout != expected.layout
            || self.binding.native_epoch != expected.native_epoch
            || self.binding.mutation_context != expected.mutation_context
            || (target.scope.element.is_none() && self.binding.content != expected.content)
            || self.target != target.scope
            || self.binding.scope.window != self.native.selected.window.window
            || self.binding.scope.pid != self.native.selected.window.pid
            || self.target.window != self.native.selected.window.window
            || self.target.pid != self.native.selected.window.pid
            || self.binding.focus_window != self.native.selected.window.window
            || self.observed_unix_ms == 0
        {
            return Err(fault(
                "agent_selected_input_target_changed",
                "The live selected native/UIA proof does not match this exact grounded action target.",
            ));
        }
        Ok(())
    }
}

/// Facts decoded from an opened object's real security descriptor. These facts
/// are used for offline policy tests, never as a substitute for querying the ACL.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrivateAclEvidence {
    pub owner_sid: String,
    pub dacl_protected: bool,
    pub ace_count: u16,
    pub access_allowed: bool,
    pub ace_flags: u8,
    pub ace_mask: u32,
    pub ace_sid: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrivateAclScope {
    Directory,
    StandaloneFile,
    /// Valid only while the caller retains and verifies the protected private
    /// parent directory handle. Standalone trust readers never select this.
    FileInPinnedPrivateDirectory,
}

impl PrivateAclEvidence {
    pub fn validate(&self, expected_sid: &str, scope: PrivateAclScope) -> Result<()> {
        validate_user_sid(expected_sid)?;
        // FILE_ALL_ACCESS, OBJECT_INHERIT_ACE | CONTAINER_INHERIT_ACE,
        // and INHERITED_ACE from the Win32 ACL contract.
        let flags_valid = match scope {
            PrivateAclScope::Directory => self.dacl_protected && self.ace_flags == 3,
            PrivateAclScope::StandaloneFile => self.dacl_protected && self.ace_flags == 0,
            PrivateAclScope::FileInPinnedPrivateDirectory => {
                (self.dacl_protected && self.ace_flags == 0) || self.ace_flags == 16
            }
        };
        if self.owner_sid != expected_sid
            || self.ace_sid != expected_sid
            || self.ace_count != 1
            || !self.access_allowed
            || self.ace_mask != 0x001f_01ff
            || !flags_valid
        {
            return Err(fault(
                "agent_private_acl_not_user_only",
                "Require this user's owner SID and exactly one full-access user ACE under the selected pinned-parent policy; never repair or widen the ACL.",
            ));
        }
        Ok(())
    }
}

pub fn validate_user_sid(sid: &str) -> Result<()> {
    let components: Vec<_> = sid.split('-').collect();
    let valid = (4..=18).contains(&components.len())
        && components[0] == "S"
        && components[1] == "1"
        && !components[2].is_empty()
        && (components[2].len() == 1 || !components[2].starts_with('0'))
        && components[2].bytes().all(|b| b.is_ascii_digit())
        && components[2].parse::<u64>().is_ok_and(|n| n < (1u64 << 48))
        && components[3..].iter().all(|part| {
            !part.is_empty()
                && (part.len() == 1 || !part.starts_with('0'))
                && part.bytes().all(|b| b.is_ascii_digit())
                && part.parse::<u32>().is_ok()
        });
    if !valid || matches!(sid, "S-1-5-18" | "S-1-5-19" | "S-1-5-20") {
        return Err(fault(
            "agent_service_identity_refused",
            "Run as the allocated interactive user, never SYSTEM/LocalService/NetworkService.",
        ));
    }
    Ok(())
}

/// A single literal file/directory name; excludes ADS, DOS aliases and traversal.
pub fn validate_private_name(name: &str) -> Result<()> {
    let stem = name
        .split('.')
        .next()
        .unwrap_or("")
        .trim_end_matches(' ')
        .to_ascii_uppercase();
    let reserved = matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL" | "CLOCK$")
        || ["COM", "LPT"].iter().any(|prefix| {
            stem.strip_prefix(prefix).is_some_and(|tail| {
                matches!(
                    tail,
                    "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
                )
            })
        });
    if name.is_empty()
        || name.encode_utf16().count() > 240
        || matches!(name, "." | "..")
        || name.ends_with(['.', ' '])
        || name
            .chars()
            .any(|c| c.is_control() || "\\/:<>\"|?*".contains(c))
        || reserved
    {
        return Err(fault(
            "agent_private_name_invalid",
            "Use a single literal filename within the approved fresh private run directory.",
        ));
    }
    Ok(())
}

/// Only local drive-absolute DOS paths. UNC, device namespaces, relative paths,
/// ADS, normalization aliases and reparse traversal are not accepted.
pub fn validate_local_dos_path(path: &str) -> Result<()> {
    let bytes = path.as_bytes();
    if path.encode_utf16().count() > 32700
        || bytes.len() < 4
        || !bytes[0].is_ascii_alphabetic()
        || &bytes[1..3] != b":\\"
        || path[3..].split('\\').count() > 64
    {
        return Err(fault(
            "agent_local_path_required",
            "Require a local drive-absolute path, never UNC/device/network paths.",
        ));
    }
    for name in path[3..].split('\\') {
        validate_private_name(name)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session() -> SessionBinding {
        SessionBinding {
            native: Identity {
                host: format!("windows-machine:{}", "1".repeat(64)),
                guest: None,
                session: "wts:2:logon:12345:luid:0:6789".into(),
                seat: "WinSta0".into(),
                desktop: "Default".into(),
            },
            user_sid: "S-1-5-21-1-2-3-1001".into(),
            session_id: 2,
            authentication_id: AuthenticationId {
                high: 0,
                low: 0x6789,
            },
            logon_time_100ns: 12345,
            input_desktop: "Default".into(),
        }
    }

    fn process() -> ProcessBinding {
        let s = session();
        ProcessBinding {
            pid: 456,
            creation_filetime_100ns: 23456,
            user_sid: s.user_sid,
            session_id: s.session_id,
            authentication_id: s.authentication_id,
            image_path: r"C:\Users\owner\run\rcu-mcp.exe".into(),
            image_sha256: "2".repeat(64),
        }
    }

    #[test]
    fn serialized_native_evidence_is_strict_and_not_guest_attestation() {
        let s = session();
        s.validate().unwrap();
        let owner = OwnerProof {
            session: s,
            process: process(),
        };
        owner.validate().unwrap();
        let mut value = serde_json::to_value(&owner).unwrap();
        assert_eq!(
            serde_json::from_value::<OwnerProof>(value.clone()).unwrap(),
            owner
        );
        value["controller_host"] = "mac".into();
        assert!(serde_json::from_value::<OwnerProof>(value).is_err());
        let mut value = serde_json::to_value(&owner).unwrap();
        value["process"]["trusted"] = true.into();
        assert!(serde_json::from_value::<OwnerProof>(value).is_err());
    }

    #[test]
    fn session_zero_services_lock_and_every_identity_change_refused() {
        let s = session();
        for sid in [
            "S-1-5-18",
            "S-1-5-19",
            "S-1-5-20",
            "S-1-5-018",
            "S-1-5-+21",
            "S-1-+5-21",
            "S-1-05-21",
            "invalid",
        ] {
            let mut other = s.clone();
            other.user_sid = sid.into();
            assert!(other.validate().is_err(), "{sid}");
        }
        let mut variants = vec![s.clone(); 10];
        variants[0].session_id = 0;
        variants[1].authentication_id.low += 1;
        variants[2].logon_time_100ns += 1;
        variants[3].input_desktop = "Winlogon".into();
        variants[4].native.seat = "Service-0".into();
        variants[5].native.guest = Some("self-declared-guest".into());
        variants[6].native.host = format!("windows-machine:{}", "2".repeat(64));
        variants[7].user_sid = "S-1-12-1-1-2-3-4".into();
        variants[8].native.desktop = "Other".into();
        variants[8].input_desktop = "Other".into();
        variants[9].native.session.push('0');
        for other in variants {
            assert!(s.require_same(&other).is_err());
        }
    }

    #[test]
    fn pid_reuse_image_swap_and_wrong_token_not_process_ownership() {
        let s = session();
        let p = process();
        let mut variants = vec![p.clone(); 8];
        variants[0].pid += 1;
        variants[1].creation_filetime_100ns += 1;
        variants[2].user_sid = "S-1-5-21-1-2-3-1002".into();
        variants[3].session_id += 1;
        variants[4].authentication_id.low += 1;
        variants[5].image_sha256 = "3".repeat(64);
        variants[6].image_path = r"C:\other\rcu-mcp.exe".into();
        variants[7].creation_filetime_100ns = 0;
        for other in variants {
            assert!(p.require_same(&other, &s).is_err());
        }
    }

    #[test]
    fn private_paths_refuse_remote_ads_reparse_alias_inputs() {
        for name in [
            "", ".", "..", "../file", "a\\b", "a:b", "nul", "CON.txt", "COM1", "LPT¹", "a.", "a ",
            "a\0b",
        ] {
            assert!(validate_private_name(name).is_err(), "{name:?}");
        }
        for path in [
            r"\\vm\share\file",
            r"\\?\UNC\server\file",
            r"\\.\pipe\name",
            r"C:relative",
            r"/tmp/file",
            r"C:\a\..\b",
            r"C:\a\b:stream",
            r"C:\a\",
            r"C:\NUL\file",
        ] {
            assert!(validate_local_dos_path(path).is_err(), "{path}");
        }
        for name in ["candidate.json", "new run-1", ".state", "文件.txt"] {
            validate_private_name(name).unwrap();
        }
        validate_local_dos_path(r"C:\Users\owner\new run\candidate.json").unwrap();
        assert!(validate_private_name("CON .txt").is_err());
        assert!(validate_local_dos_path(&format!("C:\\{}", vec!["a"; 65].join("\\"))).is_err());
    }

    #[test]
    fn inherited_acl_is_oracle_only_and_requires_the_pinned_parent_scope() {
        let sid = "S-1-5-21-1-2-3-1001";
        let explicit = PrivateAclEvidence {
            owner_sid: sid.into(),
            dacl_protected: true,
            ace_count: 1,
            access_allowed: true,
            ace_flags: 0,
            ace_mask: 0x001f_01ff,
            ace_sid: sid.into(),
        };
        explicit
            .validate(sid, PrivateAclScope::StandaloneFile)
            .unwrap();
        explicit
            .validate(sid, PrivateAclScope::FileInPinnedPrivateDirectory)
            .unwrap();
        let mut inherited = explicit.clone();
        inherited.dacl_protected = false;
        inherited.ace_flags = 16;
        assert!(
            inherited
                .validate(sid, PrivateAclScope::StandaloneFile)
                .is_err()
        );
        assert!(inherited.validate(sid, PrivateAclScope::Directory).is_err());
        inherited
            .validate(sid, PrivateAclScope::FileInPinnedPrivateDirectory)
            .unwrap();
        let mut directory = explicit.clone();
        directory.ace_flags = 3;
        directory.validate(sid, PrivateAclScope::Directory).unwrap();
        directory.dacl_protected = false;
        assert!(directory.validate(sid, PrivateAclScope::Directory).is_err());
        let mut invalid = vec![inherited.clone(); 7];
        invalid[0].owner_sid = "S-1-5-21-1-2-3-1002".into();
        invalid[1].ace_sid = "S-1-5-18".into();
        invalid[2].ace_count = 2;
        invalid[3].access_allowed = false;
        invalid[4].ace_mask = 0x1000_0000;
        invalid[5].ace_flags = 17;
        invalid[6].ace_flags = 0;
        for facts in invalid {
            assert!(
                facts
                    .validate(sid, PrivateAclScope::FileInPinnedPrivateDirectory)
                    .is_err()
            );
        }
    }
}
