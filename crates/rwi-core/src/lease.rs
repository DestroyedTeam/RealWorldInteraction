//! Advisory desktop leases shared by cooperating RWI processes.
//! One expiry worker per engine; the dispatch guard prevents expiry during input.
use crate::{Result, fault, id};
use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    path::PathBuf,
    sync::{Arc, Condvar, Mutex},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

struct Lease {
    target: String,
    key: String,
    expires: Instant,
    _file: File,
}
impl Drop for Lease {
    fn drop(&mut self) {
        // Explicit unlock matters when another thread forks a child concurrently:
        // close alone can leave the inherited open-file description locked until exec.
        let _ = self._file.unlock();
    }
}
pub struct State {
    leases: BTreeMap<String, Lease>,
    stopping: bool,
}
impl State {
    fn expire(&mut self) {
        let now = Instant::now();
        self.leases.retain(|_, l| l.expires > now);
    }
    pub fn check(&self, token: &str, target: &str, key: &str) -> Result<()> {
        match self.leases.get(token) {
            Some(l) if l.target == target && l.key == key && l.expires > Instant::now() => Ok(()),
            _ => Err(fault(
                "lease_required",
                "Claim this exact target before dispatch.",
            )),
        }
    }
    pub fn deadline(&self, token: &str, target: &str, key: &str) -> Result<Instant> {
        self.check(token, target, key)?;
        Ok(self.leases.get(token).expect("checked lease").expires)
    }
}
pub struct LeaseBook {
    root: PathBuf,
    state: Arc<Mutex<State>>,
    wake: Arc<Condvar>,
    worker: Option<JoinHandle<()>>,
}
impl LeaseBook {
    pub fn new(root: PathBuf) -> Self {
        let state = Arc::new(Mutex::new(State {
            leases: BTreeMap::new(),
            stopping: false,
        }));
        let wake = Arc::new(Condvar::new());
        let s = state.clone();
        let w = wake.clone();
        let worker = thread::spawn(move || {
            let Ok(mut g) = s.lock() else { return };
            loop {
                g.expire();
                if g.stopping {
                    return;
                }
                let wait = g
                    .leases
                    .values()
                    .map(|l| l.expires.saturating_duration_since(Instant::now()))
                    .min();
                g = match wait {
                    Some(d) => match w.wait_timeout(g, d) {
                        Ok((g, _)) => g,
                        Err(_) => return,
                    },
                    None => match w.wait(g) {
                        Ok(g) => g,
                        Err(_) => return,
                    },
                };
            }
        });
        Self {
            root: root.join("leases"),
            state,
            wake,
            worker: Some(worker),
        }
    }
    pub fn shared(&self) -> Arc<Mutex<State>> {
        self.state.clone()
    }
    pub fn claim(&mut self, target: &str, key: &str, ttl_ms: u64) -> Result<String> {
        if !(100..=60_000).contains(&ttl_ms) {
            return Err(fault("invalid_lease_ttl", "Use 100..60000 milliseconds."));
        }
        let mut state = self
            .state
            .lock()
            .map_err(|_| fault("lease_error", "Restart the server."))?;
        state.expire();
        if state.leases.values().any(|l| l.key == key) {
            return Err(fault(
                "desktop_busy",
                "Another lease owns this desktop; do not steal it.",
            ));
        }
        fs::create_dir_all(&self.root)?;
        // Lock files are never unlinked: replacing an inode could split lock ownership.
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(self.root.join(format!("{key}.lock")))?;
        file.try_lock().map_err(|_| {
            fault(
                "desktop_busy",
                "Another cooperating process owns this desktop.",
            )
        })?;
        let token = id();
        state.leases.insert(
            token.clone(),
            Lease {
                target: target.into(),
                key: key.into(),
                expires: Instant::now() + Duration::from_millis(ttl_ms),
                _file: file,
            },
        );
        self.wake.notify_one();
        Ok(token)
    }
    pub fn release(&mut self, token: &str) -> Result<()> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| fault("lease_error", "Restart the server."))?;
        if state.leases.remove(token).is_none() {
            return Err(fault(
                "unknown_lease",
                "Lease expired or belongs to another connection.",
            ));
        }
        self.wake.notify_one();
        Ok(())
    }
    pub fn active(&self) -> usize {
        self.state
            .lock()
            .map(|mut s| {
                s.expire();
                s.leases.len()
            })
            .unwrap_or(0)
    }
}
impl Drop for LeaseBook {
    fn drop(&mut self) {
        if let Ok(mut s) = self.state.lock() {
            s.stopping = true;
            s.leases.clear();
        }
        self.wake.notify_one();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
