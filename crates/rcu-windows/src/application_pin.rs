//! Portable policy used by the native opens, not a simulated Windows ACL oracle.
use super::{MAX_PATH_COMPONENTS, disk_path};
use rcu_core::{Fault, Result, fault};
use serde::Serialize;

// Exact Win32 values. The Windows build asserts the SDK aliases at its call site.
pub(super) const DIRECTORY_ACCESS: u32 = 0x20; // FILE_TRAVERSE / FILE_EXECUTE
pub(super) const FILE_ACCESS: u32 = 1; // FILE_READ_DATA
pub(super) const SHARE: u32 = 1; // FILE_SHARE_READ, no WRITE or DELETE sharing
pub(super) const DISPOSITION: u32 = 3; // OPEN_EXISTING
pub(super) const DIRECTORY_FLAGS: u32 = 0x0220_0000; // BACKUP_SEMANTICS | OPEN_REPARSE_POINT
pub(super) const FILE_FLAGS: u32 = 0x0820_0000; // SEQUENTIAL_SCAN | OPEN_REPARSE_POINT
const DIRECTORY: u32 = 0x10;
const REPARSE: u32 = 0x400;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum PinKind {
    AncestorDirectory,
    AuthorizedFile,
}

pub(super) fn canonical_path(kind: PinKind, path: &str) -> Result<String> {
    let unprefixed = path.strip_prefix(r"\\?\").unwrap_or(path);
    let b = unprefixed.as_bytes();
    if kind == PinKind::AncestorDirectory
        && b.len() == 3
        && b[0].is_ascii_alphabetic()
        && b[1..] == *b":\\"
    {
        return Ok(unprefixed.into());
    }
    disk_path(path)
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct PinRequest {
    pub kind: PinKind,
    pub authorized_file: String,
    pub path: String,
    pub native_path: String,
    pub ancestor_index: Option<usize>,
    pub ancestor_count: usize,
    pub desired_access: u32,
    pub share_mode: u32,
    pub creation_disposition: u32,
    pub flags: u32,
}
impl PinRequest {
    pub fn fault(&self, code: &str, api: &str, stage: &str, next: &str) -> Fault {
        let mut error = fault(code, next);
        error.diagnostic = Some(Box::new(serde_json::json!({
            "schema": "rcu.windows.pin.v1", "api": api, "stage": stage,
            "request": self, "native_error": null,
            "permission_fallback_attempted": false, "acl_or_privileges_modified": false,
        })));
        error
    }
    pub fn check(
        &self,
        initial: &PinStamp,
        current: &PinStamp,
        path: &str,
        stage: &str,
    ) -> Result<()> {
        let canonical = canonical_path(self.kind, path).map_err(|_| {
            self.fault(
                "application_pin_path_invalid",
                "GetFinalPathNameByHandleW",
                stage,
                "The pinned handle's path is not an unambiguous local drive path; no dispatch.",
            )
        })?;
        let identity_matches = match self.kind {
            PinKind::AuthorizedFile => initial == current,
            PinKind::AncestorDirectory => {
                // Child updates may alter directory bytes/mtime/archive bits.
                // None is a directory object identity; do not HOLD on those alone.
                initial.volume == current.volume
                    && initial.index == current.index
                    && initial.created == current.created
            }
        };
        let type_valid = match self.kind {
            PinKind::AncestorDirectory => current.attributes & (DIRECTORY | REPARSE) == DIRECTORY,
            PinKind::AuthorizedFile => current.attributes & (DIRECTORY | REPARSE) == 0,
        };
        if !identity_matches || !type_valid || !canonical.eq_ignore_ascii_case(&self.path) {
            let mut error = self.fault(
                "application_pin_changed",
                "handle_identity_and_final_path",
                stage,
                "Pinned identity, final path or non-reparse type changed; no launch is allowed.",
            );
            if let Some(d) = error.diagnostic.as_deref_mut() {
                d["initial_stamp"] = serde_json::to_value(initial).expect("pin stamp");
                d["current_stamp"] = serde_json::to_value(current).expect("pin stamp");
                d["current_path"] = canonical.into();
            }
            return Err(error);
        }
        Ok(())
    }
}

pub(super) struct PinPlan {
    pub directories: Vec<PinRequest>,
    pub file: PinRequest,
}
impl PinPlan {
    /// Used only after native managed-package policy verification. The root and
    /// descendants still receive exactly the strong pin flags; only ancestors
    /// above that registered root use the explicit OS-package trust contract.
    pub fn within_registered_package_root(path: &str, install_root: &str) -> Result<Self> {
        let root = disk_path(install_root)?;
        let mut plan = Self::new(path)?;
        let Some(start) = plan
            .directories
            .iter()
            .position(|d| d.path.eq_ignore_ascii_case(&root))
        else {
            return Err(fault(
                "observed_package_path_outside_root",
                "The exact image must be a descendant of the granted registered package root.",
            ));
        };
        plan.directories.drain(..start);
        Ok(plan)
    }
    pub fn new(path: &str) -> Result<Self> {
        let path = disk_path(path)?;
        let mut directories = Vec::with_capacity(MAX_PATH_COMPONENTS);
        let ancestor_count = path.matches('\\').count();
        for (ancestor_index, (end, _)) in path.match_indices('\\').enumerate() {
            let dir = if end == 2 { &path[..3] } else { &path[..end] };
            directories.push(PinRequest {
                kind: PinKind::AncestorDirectory,
                authorized_file: path.clone(),
                path: dir.into(),
                native_path: format!(r"\\?\{dir}"),
                ancestor_index: Some(ancestor_index),
                ancestor_count,
                desired_access: DIRECTORY_ACCESS,
                share_mode: SHARE,
                creation_disposition: DISPOSITION,
                flags: DIRECTORY_FLAGS,
            });
        }
        Ok(Self {
            directories,
            file: PinRequest {
                kind: PinKind::AuthorizedFile,
                native_path: format!(r"\\?\{path}"),
                path: path.clone(),
                authorized_file: path,
                ancestor_index: None,
                ancestor_count,
                desired_access: FILE_ACCESS,
                share_mode: SHARE,
                creation_disposition: DISPOSITION,
                flags: FILE_FLAGS,
            },
        })
    }
    // Production calls this with native CreateFile + immediate stamp/path proof.
    // Failed opens/queries drop every earlier handle. No weaker retry exists.
    pub fn open_directories<T>(
        &self,
        open: impl FnMut(&PinRequest) -> Result<T>,
    ) -> Result<Vec<T>> {
        self.directories.iter().map(open).collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(super) struct PinStamp {
    pub volume: u32,
    pub index: u64,
    pub bytes: u64,
    pub modified: u64,
    pub created: u64,
    pub attributes: u32,
}

#[cfg(test)]
mod tests {
    use super::super::ReleaseGuard;
    use super::*;
    use std::{cell::RefCell, rc::Rc};
    const PACKAGED: &str = r"C:\Program Files\WindowsApps\Microsoft.WindowsNotepad_11.2607.14.0_arm64__8wekyb3d8bbwe\Notepad\Notepad.exe";
    fn stamp() -> PinStamp {
        PinStamp {
            volume: 1,
            index: 2,
            bytes: 0,
            modified: 3,
            created: 4,
            attributes: DIRECTORY,
        }
    }
    #[test]
    fn package_scope_retains_root_descendant_pins_without_mutating_strong_plan() {
        let root = r"C:\Program Files\WindowsApps\Microsoft.WindowsNotepad_11.2607.14.0_arm64__8wekyb3d8bbwe";
        let p = PinPlan::within_registered_package_root(PACKAGED, root).unwrap();
        assert_eq!(p.directories.len(), 2);
        assert_eq!(p.directories[0].path, root);
        assert_eq!(p.directories[0].ancestor_index, Some(3));
        assert_eq!(PinPlan::new(PACKAGED).unwrap().directories.len(), 5);
        for d in &p.directories {
            assert_eq!(d.desired_access, DIRECTORY_ACCESS);
            assert_eq!(d.share_mode, SHARE);
            assert_eq!(d.flags, DIRECTORY_FLAGS);
        }
        for wrong in [
            r"C:\",
            r"D:\Other",
            r"C:\Program Files\WindowsApp",
            r"C:\Program Files\WindowsApps\Microsoft.WindowsNotepad_11",
            PACKAGED,
        ] {
            assert!(
                PinPlan::within_registered_package_root(PACKAGED, wrong).is_err(),
                "{wrong}"
            );
        }
    }
    #[test]
    fn root_normalization_is_directory_only_and_no_device_escape() {
        for root in [r"C:\", r"\\?\C:\", r"d:\"] {
            assert!(canonical_path(PinKind::AncestorDirectory, root).is_ok());
            assert!(canonical_path(PinKind::AuthorizedFile, root).is_err());
        }
        for bad in [
            r"\\?\UNC\host\dir",
            r"\\.\C:\",
            r"\\?\GLOBALROOT\Device\x",
            r"C:\..",
            r"C:\foo\",
            r"C:\foo:stream",
        ] {
            assert!(
                canonical_path(PinKind::AncestorDirectory, bad).is_err(),
                "{bad}"
            );
        }
    }
    #[test]
    fn selected_launcher_and_observed_packaged_image_have_separate_exact_plans() {
        for path in [
            r"C:\WINDOWS\System32\notepad.exe",
            PACKAGED,
            r"D:\Protected\Other Parent\baseline.txt",
        ] {
            let p = PinPlan::new(path).unwrap();
            assert_eq!(p.file.path, path);
            assert_eq!(p.file.desired_access, 1);
            assert_eq!(p.file.flags, FILE_FLAGS);
            assert_eq!(p.directories[0].path, &path[..3]);
            assert_eq!(
                p.directories.last().unwrap().path,
                path.rsplit_once('\\').unwrap().0
            );
            for (i, d) in p.directories.iter().enumerate() {
                assert_eq!(d.authorized_file, path);
                assert_eq!(d.ancestor_index, Some(i));
                assert_eq!(d.ancestor_count, p.directories.len());
                assert_eq!(d.desired_access, 0x20); // never list/read-attributes/generic-read
                assert_eq!(d.share_mode, 1); // never share-write/delete
                assert_eq!(d.creation_disposition, 3);
                assert_eq!(d.flags, DIRECTORY_FLAGS);
                assert_eq!(d.native_path, format!(r"\\?\{}", d.path));
            }
        }
    }
    #[test]
    fn denial_at_every_ancestor_is_precise_no_fallback_and_releases_prior_pins_once() {
        let p = PinPlan::new(PACKAGED).unwrap();
        for denied in 0..p.directories.len() {
            let opened = Rc::new(RefCell::new(vec![]));
            let closed = Rc::new(RefCell::new(vec![]));
            let result = p.open_directories(|r| {
                let i = r.ancestor_index.unwrap();
                opened.borrow_mut().push((i, r.desired_access));
                if i == denied {
                    let mut e = r.fault(
                        "application_directory_pin_failed",
                        "CreateFileW",
                        "open",
                        "Fixture AccessDenied; not a Windows ACL execution.",
                    );
                    e.diagnostic.as_deref_mut().unwrap()["native_error"] =
                        serde_json::json!({"hresult":"0x80070005","win32_code":5});
                    return Err(e);
                }
                let released = closed.clone();
                Ok(ReleaseGuard::new(i, move |n| {
                    released.borrow_mut().push(n);
                    true
                }))
            });
            let e = result.err().unwrap();
            assert_eq!(e.code, "application_directory_pin_failed");
            let d = e.diagnostic.unwrap();
            assert_eq!(d["request"]["path"], p.directories[denied].path);
            assert_eq!(d["request"]["desired_access"], 0x20);
            assert_eq!(d["native_error"]["win32_code"], 5);
            assert_eq!(
                *opened.borrow(),
                (0..=denied).map(|i| (i, 0x20)).collect::<Vec<_>>()
            );
            let mut actual = closed.borrow().clone();
            actual.sort();
            assert_eq!(actual, (0..denied).collect::<Vec<_>>());
        }
    }
    #[test]
    fn minimum_access_policy_does_not_require_directory_listing_in_fixture() {
        let p = PinPlan::new(PACKAGED).unwrap();
        // Synthetic access oracle only: models an ACL permitting traverse but not
        // READ_ATTRIBUTES or LIST_DIRECTORY; cannot prove the real protected ACL.
        let pins = p
            .open_directories(|r| {
                if r.desired_access & !DIRECTORY_ACCESS != 0 {
                    Err(r.fault(
                        "fixture_access_denied",
                        "fixture",
                        "open",
                        "Overbroad access",
                    ))
                } else {
                    Ok(r.path.clone())
                }
            })
            .unwrap();
        assert_eq!(pins.len(), p.directories.len());
    }
    #[test]
    fn directory_revalidation_rejects_identity_path_reparse_and_type_changes() {
        let p = PinPlan::new(PACKAGED).unwrap();
        for r in &p.directories {
            let base = stamp();
            r.check(&base, &base, &r.native_path, "before_hash")
                .unwrap();
            for phase in ["before_hash", "after_hash", "reuse", "before_dispatch"] {
                for change in 0..5 {
                    let mut now = base.clone();
                    match change {
                        0 => now.volume += 1,
                        1 => now.index += 1,
                        2 => now.created += 1,
                        3 => now.attributes |= REPARSE,
                        _ => now.attributes &= !DIRECTORY,
                    }
                    let e = r.check(&base, &now, &r.path, phase).unwrap_err();
                    assert_eq!(e.diagnostic.unwrap()["stage"], phase);
                }
                assert!(
                    r.check(&base, &base, &r.path.replacen("C:", "D:", 1), phase)
                        .is_err()
                );
            }
        }
    }
    #[test]
    fn unrelated_directory_child_updates_do_not_replace_identity_but_file_stamp_is_exact() {
        let p = PinPlan::new(PACKAGED).unwrap();
        let base = stamp();
        let mut now = base.clone();
        now.bytes += 5;
        now.modified += 6;
        now.attributes |= 0x20;
        p.directories[0]
            .check(&base, &now, r"C:\", "reuse")
            .unwrap();
        let mut file = base.clone();
        file.attributes = 0x20;
        p.file.check(&file, &file, PACKAGED, "reuse").unwrap();
        let mut changed = file.clone();
        changed.modified += 1;
        assert!(p.file.check(&file, &changed, PACKAGED, "reuse").is_err());
        changed = file.clone();
        changed.attributes |= REPARSE;
        assert!(p.file.check(&file, &changed, PACKAGED, "reuse").is_err());
    }
}
