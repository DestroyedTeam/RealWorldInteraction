//! Persist one reservation before each side effect; never forget/re-dispatch IDs.
mod recovery;
use rcu_core::device::DeviceResult;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    fs::{self, OpenOptions},
    io::{self, Read, Write},
    path::{Path, PathBuf},
};

pub enum Reservation {
    New(String),
    Existing(DeviceResult),
}
pub struct Ledger {
    path: PathBuf,
    entries: BTreeMap<String, Value>,
}
impl Ledger {
    pub fn open(path: PathBuf) -> io::Result<Self> {
        fs::create_dir_all(path.join("results"))?;
        fs::create_dir_all(path.join("recoveries"))?;
        let mut entries = BTreeMap::new();
        for e in fs::read_dir(&path)? {
            let e = e?;
            if entries.len() >= 100_000 {
                return Err(io::Error::other("operation_storage_capacity"));
            }
            let name = e
                .file_name()
                .into_string()
                .map_err(|_| io::Error::other("ledger_name"))?;
            if (name == "results" || name == "recoveries") && e.file_type()?.is_dir() {
                continue;
            }
            if name.len() != 69
                || !name.ends_with(".json")
                || !name[..64].bytes().all(|b| b.is_ascii_hexdigit())
                || !e.file_type()?.is_file()
                || e.metadata()?.len() > 65536
            {
                return Err(io::Error::other("ledger_corrupt"));
            }
            let value = bounded_json(&e.path())?;
            let principal = value["principal"]
                .as_str()
                .filter(|p| !p.is_empty() && p.len() <= 256);
            let operation = value["operation_id"]
                .as_str()
                .filter(|s| rcu_core::device::valid_id(s));
            let monitor = value["monitor_id"]
                .as_str()
                .filter(|s| rcu_core::device::valid_id(s));
            let domain = value["resource_domain"]
                .as_str()
                .filter(|s| !s.is_empty() && s.len() <= 4096);
            let hash = value["payload_sha"]
                .as_str()
                .filter(|s| s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit()));
            if principal.is_none()
                || operation.is_none()
                || monitor.is_none()
                || domain.is_none()
                || hash.is_none()
                || value["state"] != "reserved_before_dispatch"
                || rcu_core::digest(&serde_json::to_vec(&(
                    principal.unwrap(),
                    operation.unwrap(),
                ))?) != name[..64]
            {
                return Err(io::Error::other("ledger_reservation_corrupt"));
            }
            entries.insert(name[..64].to_owned(), value);
        }
        Ok(Self { path, entries })
    }
    pub fn reserve(
        &mut self,
        principal: &str,
        operation: &str,
        payload: &Value,
        monitor_id: &str,
        domain: &str,
    ) -> io::Result<Reservation> {
        let key = rcu_core::digest(&serde_json::to_vec(&(principal, operation))?);
        let payload_sha = rcu_core::digest(&serde_json::to_vec(payload)?);
        if let Some(old) = self.entries.get(&key) {
            return Ok(Reservation::Existing(
                if old["payload_sha"] != payload_sha {
                    DeviceResult::error(
                        "operation_payload_conflict",
                        "This operation ID already identifies different arguments.",
                    )
                } else {
                    DeviceResult::value(
                        json!({"operation_id":operation,"state":if !self.finished(&key).is_null(){"completed"}else{"reserved_or_effect_unknown"}, "dispatched_again":false,
                    "goal_verified":false,"record":old,"result":self.finished(&key),"automatic_replay":false}),
                    )
                },
            ));
        }
        if self.entries.len() >= 100_000 {
            return Err(io::Error::other("operation_storage_capacity"));
        }
        if self
            .entries
            .values()
            .any(|v| v["principal"] == principal && v["monitor_id"] == monitor_id)
        {
            return Err(io::Error::other("monitor_id_reused"));
        }
        let value = json!({"principal":principal,"resource_domain":domain,"monitor_id":monitor_id,"operation_id":operation,"payload_sha":payload_sha,"state":"reserved_before_dispatch"});
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(self.path.join(format!("{key}.json")))?;
        file.write_all(&serde_json::to_vec(&value)?)?;
        file.sync_all()?;
        #[cfg(unix)]
        fs::File::open(&self.path)?.sync_all()?;
        self.entries.insert(key.clone(), value);
        Ok(Reservation::New(key))
    }
    pub fn status(&self, principal: &str, operation: &str) -> Option<Value> {
        let key = rcu_core::digest(&serde_json::to_vec(&(principal, operation)).ok()?);
        let (key, value) = self.entries.get_key_value(&key).or_else(|| {
            self.entries
                .iter()
                .find(|(_, v)| v["principal"] == principal && v["monitor_id"] == operation)
        })?;
        Some(json!({"reservation":value,"result":self.finished(key),"replay_allowed":false}))
    }

    fn finished(&self, key: &str) -> Value {
        let path = self.path.join("results").join(format!("{key}.json"));
        let Ok(meta) = fs::symlink_metadata(&path) else {
            return Value::Null;
        };
        if !meta.is_file() || meta.len() > 65536 {
            return Value::Null;
        }
        bounded_json(&path)
            .ok()
            .filter(|v| serde_json::from_value::<DeviceResult>(v.clone()).is_ok())
            .unwrap_or(Value::Null)
    }
    fn pending_snapshot(&self, domain: &str) -> String {
        let keys = self
            .entries
            .iter()
            .filter(|(_, v)| v["resource_domain"] == domain)
            .filter(|(k, _)| {
                let r = self.finished(k);
                r.is_null() || r["data"]["input_domain_fenced"] == true
            })
            .map(|(k, _)| k)
            .collect::<Vec<_>>();
        if keys.is_empty() {
            return String::new();
        }
        rcu_core::digest(&serde_json::to_vec(&(domain, keys)).unwrap())
    }
    pub fn finish(&self, key: &str, result: &DeviceResult) -> io::Result<()> {
        if !self.entries.contains_key(key) {
            return Err(io::Error::other("unreserved_result"));
        }
        let raw = serde_json::to_vec(result)?;
        if raw.len() > 65536 {
            return Err(io::Error::other("result_storage_limit"));
        }
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(self.path.join("results").join(format!("{key}.json")))?;
        file.write_all(&raw)?;
        file.sync_all()?;
        #[cfg(unix)]
        fs::File::open(self.path.join("results"))?.sync_all()?;
        Ok(())
    }
}

fn bounded_json(path: &Path) -> io::Result<Value> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_file() || metadata.len() > 65536 {
        return Err(io::Error::other("ledger_file_type_or_size"));
    }
    let mut raw = Vec::new();
    fs::File::open(path)?.take(65537).read_to_end(&mut raw)?;
    if raw.len() > 65536 {
        return Err(io::Error::other("ledger_file_size"));
    }
    Ok(serde_json::from_slice(&raw)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reconciliation_is_per_domain_durable_and_never_erases_old_reservations() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("ops");
        let mut l = Ledger::open(path.clone()).unwrap();
        l.reserve("p", "uncertain-a", &json!({"x":1}), "a", "desktop-a")
            .unwrap();
        l.reserve("p", "uncertain-b", &json!({"x":2}), "b", "desktop-b")
            .unwrap();
        assert!(l.fenced("desktop-a"));
        assert!(l.fenced("desktop-b"));
        l.reconcile("desktop-a", json!({"source":"unit_test_only"}))
            .unwrap();
        assert!(!l.fenced("desktop-a"));
        assert!(l.fenced("desktop-b"));
        drop(l);
        let mut l = Ledger::open(path).unwrap();
        assert!(!l.fenced("desktop-a"));
        assert!(matches!(
            l.reserve("p", "uncertain-a", &json!({"x":1}), "a", "desktop-a")
                .unwrap(),
            Reservation::Existing(_)
        ));
        let Reservation::New(k) = l
            .reserve("p", "new-known", &json!({}), "c", "desktop-a")
            .unwrap()
        else {
            panic!("new operation")
        };
        l.finish(
            &k,
            &DeviceResult::value(json!({"input_domain_fenced":false})),
        )
        .unwrap();
        assert!(
            !l.fenced("desktop-a"),
            "a known completed operation must not invalidate recovery"
        );
        l.reserve("p", "new-unknown", &json!({}), "d", "desktop-a")
            .unwrap();
        assert!(
            l.fenced("desktop-a"),
            "a new unknown effect requires new reconciliation"
        );
    }
    #[test]
    fn corrupt_reservation_or_recovery_never_unfences() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("ops");
        let mut l = Ledger::open(path.clone()).unwrap();
        let Reservation::New(k) = l.reserve("p", "one", &json!({}), "one", "desktop").unwrap()
        else {
            panic!()
        };
        let snapshot = l.pending_snapshot("desktop");
        fs::write(
            path.join("recoveries").join(format!("{snapshot}.json")),
            b"{}",
        )
        .unwrap();
        assert!(l.fenced("desktop"));
        fs::write(path.join(format!("{k}.json")), b"{}").unwrap();
        assert!(Ledger::open(path).is_err());
    }
    #[test]
    fn restart_and_changed_payload_never_dispatch_again() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("ops");
        let mut ledger = Ledger::open(path.clone()).unwrap();
        assert!(matches!(
            ledger
                .reserve("p", "one", &json!({"x":1}), "one", "desktop")
                .unwrap(),
            Reservation::New(_)
        ));
        drop(ledger);
        let mut ledger = Ledger::open(path).unwrap();
        assert!(matches!(
            ledger
                .reserve("p", "one", &json!({"x":1}), "one", "desktop")
                .unwrap(),
            Reservation::Existing(_)
        ));
        match ledger
            .reserve("p", "one", &json!({"x":2}), "one", "desktop")
            .unwrap()
        {
            Reservation::Existing(r) => assert!(r.is_error),
            _ => panic!("replayed"),
        }
        assert!(matches!(
            ledger
                .reserve("other", "one", &json!({"x":1}), "one", "desktop")
                .unwrap(),
            Reservation::New(_)
        ));
    }
}
