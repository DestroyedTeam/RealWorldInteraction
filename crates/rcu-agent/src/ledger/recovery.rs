//! Recovery persistence, separated from operation reservations.
use super::*;
impl Ledger {
    pub fn fenced(&self, domain: &str) -> bool {
        let snapshot = self.pending_snapshot(domain);
        if snapshot.is_empty() {
            return false;
        }
        let path = self
            .path
            .join("recoveries")
            .join(format!("{snapshot}.json"));
        let data = bounded_json(&path).ok();
        data.is_none_or(|r| {
            r["resource_domain"] != domain
                || r["snapshot"] != snapshot
                || r["resource_checks_passed"] != true
        })
    }
    pub fn reconcile(&self, domain: &str, proof: Value) -> io::Result<Value> {
        let snapshot = self.pending_snapshot(domain);
        let record = json!({"resource_domain":domain,"snapshot":snapshot,"resource_checks_passed":true,"proof":proof,"old_operations_replayable":false,"business_goal_verified":false});
        if snapshot.is_empty() || !self.fenced(domain) {
            return Ok(record);
        }
        if fs::read_dir(self.path.join("recoveries"))?
            .take(257)
            .count()
            >= 256
        {
            return Err(io::Error::other("recovery_storage_limit"));
        }
        let mut file = OpenOptions::new().write(true).create_new(true).open(
            self.path
                .join("recoveries")
                .join(format!("{snapshot}.json")),
        )?;
        file.write_all(&serde_json::to_vec(&record)?)?;
        file.sync_all()?;
        #[cfg(unix)]
        fs::File::open(self.path.join("recoveries"))?.sync_all()?;
        Ok(record)
    }
}
