//! Append-only recovery commits. Incomplete attempts are retained but never trusted.
use super::*;

const RECOVERY_LIMIT: usize = 256;

fn valid_record(path: &Path, domain: &str, snapshot: &str) -> bool {
    bounded_json(path).is_ok_and(|r| {
        r["resource_domain"] == domain
            && r["snapshot"] == snapshot
            && r["resource_checks_passed"] == true
            && r["old_operations_replayable"] == false
            && r["business_goal_verified"] == false
            && r["proof"].is_object()
    })
}

impl Ledger {
    pub fn fenced(&self, domain: &str) -> bool {
        let snapshot = self.pending_snapshot(domain);
        if snapshot.is_empty() {
            return false;
        }
        let Ok(entries) = fs::read_dir(self.path.join("recoveries")) else {
            return true;
        };
        let prefix = format!("v2-{snapshot}-");
        let mut found = false;
        for (count, entry) in entries.enumerate() {
            if count >= RECOVERY_LIMIT {
                return true;
            }
            let Ok(entry) = entry else {
                return true;
            };
            let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
                return true;
            };
            // Preserve valid v1 records for compatibility; malformed v1 records
            // can be superseded by a new v2 commit, never overwritten/deleted.
            let committed = name == format!("{snapshot}.json")
                || name
                    .strip_prefix(&prefix)
                    .and_then(|s| s.strip_suffix(".json"))
                    .is_some_and(|s| uuid::Uuid::parse_str(s).is_ok());
            if committed && valid_record(&entry.path(), domain, &snapshot) {
                found = true;
            }
        }
        !found
    }

    pub fn reconcile(&self, domain: &str, proof: Value) -> io::Result<Value> {
        let snapshot = self.pending_snapshot(domain);
        let attempt = uuid::Uuid::new_v4().to_string();
        let record = json!({"format":"rcu.recovery.v2","attempt":attempt,"resource_domain":domain,"snapshot":snapshot,"resource_checks_passed":true,"proof":proof,"old_operations_replayable":false,"business_goal_verified":false});
        if snapshot.is_empty() || !self.fenced(domain) {
            return Ok(record);
        }
        let directory = self.path.join("recoveries");
        // Reserve space for both temporary and committed links. Bad/abandoned
        // attempts count towards the documented limit; never silently delete them.
        if fs::read_dir(&directory)?.take(RECOVERY_LIMIT).count() >= RECOVERY_LIMIT - 1 {
            return Err(io::Error::other("recovery_storage_limit"));
        }
        let raw = serde_json::to_vec(&record)?;
        if raw.len() > 65536 || !record["proof"].is_object() {
            return Err(io::Error::other("recovery_proof_format_or_size"));
        }
        let stem = format!("v2-{snapshot}-{attempt}");
        let pending = directory.join(format!("{stem}.pending"));
        let committed = directory.join(format!("{stem}.json"));
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&pending)?;
        #[cfg(test)]
        crash_point("created");
        #[cfg(test)]
        if crash_selected("partial") {
            file.write_all(&raw[..raw.len() / 2])?;
            crash_point("partial");
        }
        file.write_all(&raw)?;
        #[cfg(test)]
        crash_point("written");
        file.sync_all()?;
        drop(file);
        #[cfg(test)]
        crash_point("file_synced");
        // An atomic new hard link publishes a fully written, synced record;
        // unlike rename, it cannot overwrite any existing committed evidence.
        fs::hard_link(&pending, &committed)?;
        #[cfg(test)]
        crash_point("published");
        #[cfg(unix)]
        fs::File::open(&directory)?.sync_all()?;
        #[cfg(test)]
        crash_point("directory_synced");
        // Only our successfully committed temporary link is disposable. Failure
        // to unlink it does not invalidate the durable committed link.
        let _ = fs::remove_file(&pending);
        Ok(record)
    }
}

#[cfg(test)]
fn crash_selected(stage: &str) -> bool {
    std::env::var("RCU_TEST_RECOVERY_CRASH_STAGE").as_deref() == Ok(stage)
}
#[cfg(test)]
fn crash_point(stage: &str) {
    if crash_selected(stage) {
        std::process::exit(73);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn crash_child() {
        let Some(path) = std::env::var_os("RCU_TEST_RECOVERY_CRASH_PATH") else {
            return;
        };
        Ledger::open(path.into())
            .unwrap()
            .reconcile("desktop-a", json!({"source":"synthetic_resource_check"}))
            .unwrap();
        panic!("selected crash point was not reached");
    }

    #[test]
    fn process_crash_at_each_commit_boundary_can_recover_without_replaying() {
        for stage in [
            "created",
            "partial",
            "written",
            "file_synced",
            "published",
            "directory_synced",
        ] {
            let temp = tempfile::tempdir().unwrap();
            let path = temp.path().join("operations");
            let mut ledger = Ledger::open(path.clone()).unwrap();
            ledger
                .reserve("p", "request-a", &json!({}), "monitor-a", "desktop-a")
                .unwrap();
            ledger
                .reserve("p", "request-b", &json!({}), "monitor-b", "desktop-b")
                .unwrap();
            drop(ledger);
            let status = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "ledger::recovery::tests::crash_child",
                    "--nocapture",
                ])
                .env("RCU_TEST_RECOVERY_CRASH_PATH", &path)
                .env("RCU_TEST_RECOVERY_CRASH_STAGE", stage)
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
                .unwrap();
            assert_eq!(status.code(), Some(73), "{stage}");
            let mut ledger = Ledger::open(path.clone()).unwrap();
            if !matches!(stage, "published" | "directory_synced") {
                assert!(
                    ledger.fenced("desktop-a"),
                    "unpublished {stage} must not un-fence"
                );
            }
            let interrupted: Vec<_> = fs::read_dir(path.join("recoveries"))
                .unwrap()
                .map(|p| {
                    let p = p.unwrap().path();
                    let bytes = fs::read(&p).unwrap();
                    (p, bytes)
                })
                .collect();
            ledger
                .reconcile(
                    "desktop-a",
                    json!({"source":"fresh_synthetic_resource_check"}),
                )
                .unwrap();
            assert!(
                !ledger.fenced("desktop-a"),
                "fresh recovery failed after {stage}"
            );
            assert!(ledger.fenced("desktop-b"));
            for (p, bytes) in interrupted {
                assert_eq!(
                    fs::read(p).unwrap(),
                    bytes,
                    "bad/interrupted evidence must be retained"
                );
            }
            assert!(matches!(
                ledger
                    .reserve("p", "request-a", &json!({}), "monitor-a", "desktop-a")
                    .unwrap(),
                Reservation::Existing(_)
            ));
            drop(ledger);
            let ledger = Ledger::open(path).unwrap();
            assert!(!ledger.fenced("desktop-a"));
            assert!(ledger.fenced("desktop-b"));
        }
    }

    #[test]
    fn malformed_legacy_records_can_be_superseded_but_not_overwritten() {
        for malformed in [b"".as_slice(), b"{\"resource_domain\":", b"{}"] {
            let temp = tempfile::tempdir().unwrap();
            let mut ledger = Ledger::open(temp.path().join("operations")).unwrap();
            ledger
                .reserve("p", "old", &json!({}), "old", "desktop")
                .unwrap();
            let p = ledger
                .path
                .join("recoveries")
                .join(format!("{}.json", ledger.pending_snapshot("desktop")));
            fs::write(&p, malformed).unwrap();
            assert!(ledger.fenced("desktop"));
            ledger
                .reconcile("desktop", json!({"fresh_check":true}))
                .unwrap();
            assert!(!ledger.fenced("desktop"));
            assert_eq!(fs::read(p).unwrap(), malformed);
        }
    }
}
