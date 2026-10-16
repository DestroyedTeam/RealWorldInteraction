//! Offline instrumentation only: delegates all actual action/oracle semantics to
//! the frozen Fixture. The independent model file is not a MCP success sidecar.
use rwi_core::{
    Backend, Binding, Identity, Intent, Oracle, Predicate, Resolved, Result, Scene, Scope, fault,
};
use serde::{Deserialize, Serialize};
use std::{fs, io::Write, path::PathBuf};

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Config {
    effect_file: Option<PathBuf>,
    png_size: Option<[u32; 2]>,
    #[serde(default)]
    ack_without_effect: bool,
    #[serde(default)]
    disconnect_after_effect: bool,
    #[serde(default)]
    act_delay_ms: u64,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Model {
    schema: String,
    calls: u64,
    value: String,
    option: bool,
    dialog: bool,
    epoch: u64,
}
pub struct Instrumented {
    fixture: rwi_core::fixture::Fixture,
    state: PathBuf,
    png: Vec<u8>,
    size: Option<[u32; 2]>,
    delay_ms: u64,
    expires_ms: u64,
    hold_effect: bool,
    effect_file: Option<PathBuf>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LinkGrant {
    expires_unix_ms: u64,
    hold_effect_until_link_drop: bool,
}
fn bounded(path: &std::path::Path, cap: usize) -> Result<Vec<u8>> {
    use std::io::Read;
    if fs::symlink_metadata(path)?.file_type().is_symlink() {
        return Err(fault("fixture_symlink", "No state symlinks."));
    }
    let mut b = Vec::new();
    fs::File::open(path)?
        .take(cap as u64 + 1)
        .read_to_end(&mut b)?;
    if b.len() > cap {
        return Err(fault("fixture_file_limit", "Bound local fixture files."));
    }
    Ok(b)
}
fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
impl Instrumented {
    pub fn open(state: PathBuf) -> Result<Self> {
        let config_path = state.join("fixture-control.json");
        let config: Config = if config_path.exists() {
            serde_json::from_slice(&bounded(&config_path, 65536)?).map_err(|_| {
                fault(
                    "fixture_config_invalid",
                    "Exact fixture-only control schema.",
                )
            })?
        } else {
            Config::default()
        };
        if config.act_delay_ms > 2000
            || config
                .png_size
                .is_some_and(|s| s[0] == 0 || s[1] == 0 || s[0] > 2048 || s[1] > 2048)
        {
            return Err(fault(
                "fixture_config_bounds",
                "Bound fixture memory/delay.",
            ));
        }
        if let Some(path) = &config.effect_file
            && (!path.is_absolute()
                || !path.is_file()
                || fs::symlink_metadata(path)?.file_type().is_symlink()
                || path.components().any(|p| {
                    matches!(
                        p,
                        std::path::Component::ParentDir | std::path::Component::CurDir
                    )
                }))
        {
            return Err(fault(
                "fixture_effect_path",
                "Only the locally seeded test file.",
            ));
        }
        let fixture = rwi_core::fixture::Fixture::default();
        {
            let mut app = fixture.0.lock().unwrap();
            let previous = state.join("fixture-model.json");
            if previous.exists() {
                let model: Model = serde_json::from_slice(&bounded(&previous, 65536)?)
                    .map_err(|_| fault("fixture_model_invalid", "Never reset an unknown model."))?;
                if model.schema != "actual-fixture-model-v1" || model.value.len() > 16384 {
                    return Err(fault("fixture_model_invalid", "No schema fallback."));
                }
                app.calls = model.calls;
                app.value = model.value;
                app.option = model.option;
                app.dialog = model.dialog;
                app.epoch = model.epoch;
            }
            app.ack_without_effect = config.ack_without_effect;
            app.disconnect = config.disconnect_after_effect;
        }
        let png = if let Some(size) = config.png_size {
            let mut seed = 0x32618u32;
            let pixels: Vec<u8> = (0..(size[0] as usize * size[1] as usize * 3))
                .map(|_| {
                    // Deterministic image noise, NOT cryptography.
                    seed ^= seed << 13;
                    seed ^= seed >> 17;
                    seed ^= seed << 5;
                    seed as u8
                })
                .collect();
            rwi_imaging::encode_rgb(size, &pixels)?
        } else {
            Vec::new()
        };
        let link_grant = state.join("link-grant.json");
        let grant: LinkGrant = if link_grant.exists() {
            serde_json::from_slice(&bounded(&link_grant, 1024)?)
                .map_err(|_| fault("fixture_link_grant_invalid", "Fixed local link grant only."))?
        } else {
            LinkGrant {
                expires_unix_ms: u64::MAX,
                hold_effect_until_link_drop: false,
            }
        };
        let instance = Self {
            fixture,
            state,
            png,
            size: config.png_size,
            delay_ms: config.act_delay_ms,
            expires_ms: grant.expires_unix_ms,
            hold_effect: grant.hold_effect_until_link_drop,
            effect_file: config.effect_file,
        };
        instance.persist()?;
        if !instance.png.is_empty() {
            write_sync(instance.state.join("fixture-original.png"), &instance.png)?;
        }
        Ok(instance)
    }
    fn persist(&self) -> Result<()> {
        let app = self.fixture.0.lock().unwrap();
        let model = Model {
            schema: "actual-fixture-model-v1".into(),
            calls: app.calls,
            value: app.value.clone(),
            option: app.option,
            dialog: app.dialog,
            epoch: app.epoch,
        };
        write_sync(
            self.state.join("fixture-model.json"),
            &serde_json::to_vec(&model).unwrap(),
        )
    }
}
fn write_sync(path: PathBuf, bytes: &[u8]) -> Result<()> {
    let temporary = path.with_extension("pending");
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    fs::rename(&temporary, &path)?;
    fs::File::open(path.parent().unwrap())?.sync_all()?;
    Ok(())
}
impl Backend for Instrumented {
    fn identity(&self) -> Identity {
        self.fixture.identity()
    }
    fn capabilities(&self) -> serde_json::Value {
        self.fixture.capabilities()
    }
    fn observe(&mut self) -> Result<Scene> {
        let mut scene = self.fixture.observe()?;
        if let Some(size) = self.size {
            scene.image_size = size;
            scene.png = self.png.clone();
        }
        Ok(scene)
    }
    fn act(
        &mut self,
        binding: &Binding,
        target: &Resolved,
        intent: &Intent,
        value: Option<&str>,
    ) -> Result<Option<bool>> {
        if self.delay_ms != 0 {
            let _pending = rwi_core::operation::native_call("fixture-only-delayed-guard")?;
            write_sync(
                self.state.join("fixture-delay-guard-ready.json"),
                b"{\"real_operation_guard_entered\":true}",
            )?;
            std::thread::sleep(std::time::Duration::from_millis(self.delay_ms));
            rwi_core::operation::checkpoint("fixture-delay-finished")?;
        }
        if now_ms() >= self.expires_ms {
            return Err(fault(
                "fixture_task_expired",
                "No dispatch after approved task expiry.",
            ));
        }
        let effect_guard = if self.hold_effect {
            Some(rwi_core::operation::native_call(
                "fixture-post-effect-guard",
            )?)
        } else {
            None
        };
        let before = self.fixture.0.lock().unwrap().value.clone();
        let result = self.fixture.act(binding, target, intent, value);
        self.persist()?;
        let after = self.fixture.0.lock().unwrap().value.clone();
        // Independent file bytes come from the actual application's changed
        // value, including an effect whose acknowledgement was lost. No oracle
        // or expected-goal field writes this file, and ack-without-effect does
        // not change it.
        if before != after
            && let Some(path) = &self.effect_file
        {
            write_sync(path.clone(), after.as_bytes())?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
            }
        }
        if self.hold_effect {
            write_sync(
                self.state.join("fixture-effect-ready.json"),
                b"{\"effect_persisted\":true,\"terminal_response_produced\":false}",
            )?;
            let until = std::time::Instant::now() + std::time::Duration::from_secs(3);
            while !self.state.join("fixture-link-forwarding-detached").exists()
                && std::time::Instant::now() < until
            {
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            write_sync(self.state.join("fixture-effect-guard-ended.json"), &serde_json::to_vec(&serde_json::json!({
                "local_forwarding_detached_marker_seen":self.state.join("fixture-link-forwarding-detached").exists(),
                "peer_physical_eof_before_guard_release":"not_observed"})).unwrap())?;
        }
        drop(effect_guard);
        result
    }
    fn inspect(&mut self, scope: &Scope, predicate: &Predicate, expected: &str) -> Result<Oracle> {
        self.fixture.inspect(scope, predicate, expected)
    }
}
