use rwi_core::{Result, WindowRelation, fault};
use rwi_imaging::Rect;
use serde::{Deserialize, Serialize};
pub fn scoped_region(window: Rect, region: [u32; 4]) -> Result<Rect> {
    let [x, y, width, height] = region;
    if width == 0
        || height == 0
        || u64::from(x) + u64::from(width) > u64::from(window.width)
        || u64::from(y) + u64::from(height) > u64::from(window.height)
    {
        return Err(fault(
            "invalid_native_roi",
            "ROI must lie completely within the native window capture bounds; no silent clipping.",
        ));
    }
    Rect {
        x: i32::try_from(i64::from(window.x) + i64::from(x))
            .map_err(|_| fault("invalid_native_roi", "Physical x overflow."))?,
        y: i32::try_from(i64::from(window.y) + i64::from(y))
            .map_err(|_| fault("invalid_native_roi", "Physical y overflow."))?,
        width,
        height,
    }
    .checked()
}

/// Bound the serialized structured result, not merely the number of HWNDs.
pub fn catalog_page(
    header: serde_json::Value,
    rows: &[WindowRecord],
    catalog_id: &str,
    offset: usize,
    native_complete: bool,
) -> Result<serde_json::Value> {
    if offset > rows.len() {
        return Err(fault(
            "invalid_catalog_cursor",
            "Offset is outside this catalog.",
        ));
    }
    let mut result = header;
    let object = result.as_object_mut().ok_or_else(|| {
        fault(
            "invalid_catalog_header",
            "Internal discovery header is not an object.",
        )
    })?;
    object.insert("windows".into(), serde_json::json!([]));
    object.insert("matched_windows".into(), rows.len().into());
    object.insert("offset".into(), offset.into());
    object.insert("catalog_id".into(), catalog_id.into());
    object.insert("page_limit".into(), 8.into());
    object.insert("structured_byte_limit".into(), (12 * 1024).into());
    let mut end = offset;
    loop {
        result["next_cursor"] = if end < rows.len() {
            format!("{catalog_id}/{end}").into()
        } else {
            serde_json::Value::Null
        };
        result["complete"] = (native_complete && offset == 0 && end == rows.len()).into();
        result["page_count"] = (end - offset).into();
        if serde_json::to_vec(&result)
            .map_err(|_| {
                fault(
                    "discovery_encoding_failed",
                    "Native metadata encoding failed.",
                )
            })?
            .len()
            > 12 * 1024
        {
            return Err(fault(
                "discovery_metadata_budget",
                "Catalog header or a single native record exceeds 12 KiB; no silently truncated IDs.",
            ));
        }
        if end == rows.len() || end - offset == 8 {
            break;
        }
        let mut next = result.clone();
        next["windows"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::to_value(&rows[end]).map_err(|_| {
                fault(
                    "discovery_encoding_failed",
                    "Native window encoding failed.",
                )
            })?);
        next["next_cursor"] = if end + 1 < rows.len() {
            format!("{catalog_id}/{}", end + 1).into()
        } else {
            serde_json::Value::Null
        };
        if serde_json::to_vec(&next)
            .map_err(|_| {
                fault(
                    "discovery_encoding_failed",
                    "Native metadata encoding failed.",
                )
            })?
            .len()
            > 12 * 1024
        {
            if end == offset {
                return Err(fault(
                    "discovery_metadata_budget",
                    "One native window record exceeds page budget.",
                ));
            }
            break;
        }
        result = next;
        end += 1;
    }
    Ok(result)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct NativeWindowKey {
    pub window: String,
    pub pid: u32,
    pub process_created: u64,
}

/// Explicit external allocation, never a native attestation of the VM route.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InputGrant {
    pub run_id: String,
    pub route_vm_id: String,
    pub expected_identity: rwi_core::Identity,
    pub root: NativeWindowKey,
    pub expires_unix_ms: u64,
}
impl InputGrant {
    pub fn validate(&self, identity: &rwi_core::Identity, now_ms: u64) -> Result<()> {
        if self.run_id.is_empty()
            || self.run_id.len() > 80
            || !self
                .run_id
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'-')
            || (!self.route_vm_id.is_empty()
                && (self.route_vm_id.len() != 36
                    || !self.route_vm_id.bytes().enumerate().all(|(i, c)| {
                        if [8, 13, 18, 23].contains(&i) {
                            c == b'-'
                        } else {
                            c.is_ascii_hexdigit()
                        }
                    })))
            || self.root.pid == 0
            || self.root.process_created == 0
            || !self.root.window.starts_with("hwnd:")
        {
            return Err(fault(
                "invalid_input_grant",
                "Require an explicit run/VM route and exact allocated HWND/PID/creation identity.",
            ));
        }
        if &self.expected_identity != identity {
            return Err(fault(
                "input_grant_identity_mismatch",
                "Grant must exactly match native machine/logon/session/seat/desktop identity.",
            ));
        }
        if self.expires_unix_ms <= now_ms || self.expires_unix_ms - now_ms > 60 * 60 * 1000 {
            return Err(fault(
                "input_grant_expired",
                "Use a fresh explicitly allocated input slot, maximum 60 minutes; no automatic extension.",
            ));
        }
        Ok(())
    }
    pub fn allows(&self, selected: &WindowRecord) -> bool {
        selected.key().as_ref() == Some(&self.root)
            || (selected.owner_chain_complete && selected.owner_chain.contains(&self.root))
    }
}
#[derive(Debug, Clone, Serialize)]
pub struct WindowRecord {
    pub window: String,
    pub pid: u32,
    pub process_created: Option<u64>,
    pub native_epoch: Option<String>,
    pub title: String,
    pub title_truncated: bool,
    pub class: String,
    pub rectangle: Option<Rect>,
    pub dwm_visible_frame: Option<Rect>,
    pub visible: bool,
    pub enabled: bool,
    pub minimized: bool,
    pub cloaked: Option<bool>,
    pub foreground: bool,
    pub window_dpi: Option<u32>,
    pub window_dpi_awareness: i32,
    pub owner_chain: Vec<NativeWindowKey>,
    pub owner_chain_complete: bool,
    pub parent: Option<String>,
    pub z_index: usize,
}
impl WindowRecord {
    pub fn key(&self) -> Option<NativeWindowKey> {
        Some(NativeWindowKey {
            window: self.window.clone(),
            pid: self.pid,
            process_created: self.process_created?,
        })
    }
}
pub fn check_relation(
    previous: Option<&WindowRecord>,
    selected: &WindowRecord,
    relation: &WindowRelation,
) -> Result<()> {
    let valid = match (previous, relation) {
        (None, WindowRelation::Initial) => true,
        (Some(p), WindowRelation::OwnedPopup) => p
            .key()
            .is_some_and(|k| selected.owner_chain_complete && selected.owner_chain.contains(&k)),
        (Some(p), WindowRelation::ReturnToOwner) => selected
            .key()
            .is_some_and(|k| p.owner_chain_complete && p.owner_chain.contains(&k)),
        _ => false,
    };
    if !valid {
        return Err(fault(
            "window_relation_unproven",
            "Use a fresh native owner-chain transition; same titles, PID alone and geometric proximity are not ownership evidence. A new unrelated target requires a new explicit connection.",
        ));
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    fn grant() -> InputGrant {
        InputGrant {
            run_id: "test-run".into(),
            route_vm_id: "1d91ce65-b3ef-4bdd-a60c-5e556ee4ad4a".into(),
            expected_identity: rwi_core::Identity {
                host: "native-machine".into(),
                guest: None,
                session: "wts:1:logon:1".into(),
                seat: "WinSta0".into(),
                desktop: "Default".into(),
            },
            root: window("hwnd:123").key().unwrap(),
            expires_unix_ms: 2000,
        }
    }
    #[test]
    fn discovery_135_windows_is_bounded_without_silent_coverage_claim() {
        let rows = (0..135)
            .map(|n| window(&format!("hwnd:{n:x}")))
            .collect::<Vec<_>>();
        let first = catalog_page(
            serde_json::json!({"enumeration_complete":true}),
            &rows,
            "catalog",
            0,
            true,
        )
        .unwrap();
        assert_eq!(first["windows"].as_array().unwrap().len(), 8);
        assert_eq!(first["matched_windows"], 135);
        assert_eq!(first["next_cursor"], "catalog/8");
        assert_eq!(first["complete"], false);
        assert!(serde_json::to_vec(&first).unwrap().len() <= 12 * 1024);
        let last = catalog_page(serde_json::json!({}), &rows, "catalog", 128, true).unwrap();
        assert_eq!(last["windows"].as_array().unwrap().len(), 7);
        assert!(last["next_cursor"].is_null());
        assert_eq!(last["complete"], false);
        let one = catalog_page(serde_json::json!({}), &rows[..1], "filtered", 0, true).unwrap();
        assert_eq!(one["complete"], true);
    }
    #[test]
    fn discovery_byte_budget_counts_utf8_and_rejects_oversized_single_records() {
        let mut w = window("hwnd:1");
        w.title = "文".repeat(1000);
        let rows = vec![w.clone(); 8];
        let page = catalog_page(serde_json::json!({}), &rows, "utf8", 0, true).unwrap();
        assert!(page["windows"].as_array().unwrap().len() < 8);
        assert!(serde_json::to_vec(&page).unwrap().len() <= 12 * 1024);
        w.title = "x".repeat(13 * 1024);
        assert!(catalog_page(serde_json::json!({}), &[w], "large", 0, true).is_err());
        assert!(
            rwi_core::DiscoveryQuery {
                cursor: Some("x".into()),
                pid: Some(4),
                title_contains: None
            }
            .validate()
            .is_err()
        );
    }
    #[test]
    fn input_grant_binds_machine_session_and_expires_without_extension() {
        let g = grant();
        assert!(g.validate(&g.expected_identity, 1000).is_ok());
        for field in ["host", "guest", "session", "seat", "desktop"] {
            let mut id = g.expected_identity.clone();
            match field {
                "host" => id.host += "wrong",
                "guest" => id.guest = Some("guessed".into()),
                "session" => id.session += "new-logon",
                "seat" => id.seat += "wrong",
                _ => id.desktop += "secure",
            }
            assert_eq!(
                g.validate(&id, 1000).unwrap_err().code,
                "input_grant_identity_mismatch"
            );
        }
        assert!(g.validate(&g.expected_identity, 2000).is_err());
        let mut far = g.clone();
        far.expires_unix_ms = 3_601_001;
        assert!(far.validate(&g.expected_identity, 1000).is_err());
    }
    #[test]
    fn input_grant_rejects_reused_hwnd_or_unrelated_window_but_allows_native_owned_dialog() {
        let g = grant();
        let root = window("hwnd:123");
        assert!(g.allows(&root));
        let mut reused = root.clone();
        reused.process_created = Some(2);
        assert!(!g.allows(&reused));
        let mut dialog = window("hwnd:999");
        assert!(!g.allows(&dialog));
        dialog.owner_chain.push(root.key().unwrap());
        assert!(g.allows(&dialog));
        dialog.owner_chain_complete = false;
        assert!(!g.allows(&dialog));
        let mut data = serde_json::to_value(g).unwrap();
        data["allow_everything"] = true.into();
        assert!(serde_json::from_value::<InputGrant>(data).is_err());
    }
    #[test]
    fn physical_roi_preserves_negative_origin_without_double_dpi_scale() {
        let r = scoped_region(
            Rect {
                x: -3600,
                y: -2186,
                width: 3600,
                height: 2186,
            },
            [1500, 1000, 1200, 600],
        )
        .unwrap();
        assert_eq!(
            r,
            Rect {
                x: -2100,
                y: -1186,
                width: 1200,
                height: 600
            }
        );
        assert!(scoped_region(r, [1199, 0, 2, 1]).is_err());
    }
    fn window(id: &str) -> WindowRecord {
        WindowRecord {
            window: id.into(),
            pid: 42,
            process_created: Some(1),
            native_epoch: Some(id.into()),
            title: "same".into(),
            title_truncated: false,
            class: "same".into(),
            rectangle: None,
            dwm_visible_frame: None,
            visible: true,
            enabled: true,
            minimized: false,
            cloaked: Some(false),
            foreground: false,
            window_dpi: Some(288),
            window_dpi_awareness: 2,
            owner_chain: vec![],
            owner_chain_complete: true,
            parent: None,
            z_index: 0,
        }
    }
    #[test]
    fn popup_requires_native_owner_identity_not_matching_title_pid_or_bounds() {
        let p = window("a");
        let mut c = window("b");
        assert!(check_relation(Some(&p), &c, &WindowRelation::OwnedPopup).is_err());
        c.owner_chain.push(p.key().unwrap());
        assert!(check_relation(Some(&p), &c, &WindowRelation::OwnedPopup).is_ok());
        assert!(check_relation(Some(&c), &p, &WindowRelation::ReturnToOwner).is_ok());
        let mut reused = p.clone();
        reused.process_created = Some(2);
        assert!(check_relation(Some(&reused), &c, &WindowRelation::OwnedPopup).is_err());
        c.owner_chain_complete = false;
        assert!(check_relation(Some(&p), &c, &WindowRelation::OwnedPopup).is_err());
    }
    #[test]
    fn initial_cannot_reset_a_bound_connection() {
        let p = window("a");
        assert!(check_relation(None, &p, &WindowRelation::Initial).is_ok());
        assert!(check_relation(Some(&p), &p, &WindowRelation::Initial).is_err());
    }
}
