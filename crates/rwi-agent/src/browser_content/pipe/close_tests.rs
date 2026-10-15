use super::*;
use std::io::Write;
use std::{
    fs,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

const FIXTURE_ENV: &str = "RWI_F14_OWNED_CLOSE_FIXTURE";
const CHILD_TEST: &str = "browser_content::pipe::close_tests::f14_browser_graceful_child_fixture";

#[test]
fn f14_browser_graceful_child_fixture() {
    let Some(path) = std::env::var_os(FIXTURE_ENV) else {
        return;
    };
    let path = PathBuf::from(path);
    assert_eq!(
        path.parent().unwrap(),
        std::env::temp_dir().canonicalize().unwrap()
    );
    assert!(
        path.file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .starts_with("rwi-f14-close-")
    );
    assert_eq!(
        fs::read(path.join("owner")).unwrap(),
        b"own-rust-child-only"
    );
    let deadline = Instant::now() + Duration::from_secs(5);
    while !path.join("go").exists() {
        assert!(Instant::now() < deadline, "fixture parent never sent close");
        std::thread::sleep(Duration::from_millis(2));
    }
    std::thread::sleep(Duration::from_millis(450));
    fs::write(path.join("done"), b"delayed-clean-exit").unwrap();
}

#[test]
fn f14_browser_graceful_close_reaps_delayed_child_without_rpc_or_signal_replay() {
    let dir = std::env::temp_dir()
        .canonicalize()
        .unwrap()
        .join(format!("rwi-f14-close-{}", uuid::Uuid::new_v4()));
    fs::DirBuilder::new().mode(0o700).create(&dir).unwrap();
    fs::write(dir.join("owner"), b"own-rust-child-only").unwrap();
    let child = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", CHILD_TEST])
        .env(FIXTURE_ENV, &dir)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let pid = child.id();
    let uid = unsafe { libc::getuid() };
    let process_birth = birth(pid, uid).unwrap();
    let (tx, mut server_read) = UnixStream::pair().unwrap();
    let (mut server_write, rx) = UnixStream::pair().unwrap();
    tx.set_nonblocking(true).unwrap();
    rx.set_nonblocking(true).unwrap();
    server_read
        .set_read_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    server_write
        .set_write_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let server_calls = calls.clone();
    let go = dir.join("go");
    let server = std::thread::spawn(move || {
        let mut message = Vec::new();
        loop {
            let mut byte = [0];
            server_read.read_exact(&mut byte).unwrap();
            if byte[0] == 0 {
                break;
            }
            assert!(message.len() < CAP);
            message.push(byte[0]);
        }
        let request: Value = serde_json::from_slice(&message).unwrap();
        assert_eq!(request["method"], "Browser.close");
        server_calls.fetch_add(1, Ordering::SeqCst);
        fs::write(go, b"close-accepted").unwrap();
        let mut reply = serde_json::to_vec(&json!({"id":request["id"],"result":{}})).unwrap();
        reply.push(0);
        server_write.write_all(&reply).unwrap();
    });
    let mut owned = OwnedChrome {
        child: Some(child),
        pipe: Pipe {
            tx,
            rx,
            framer: Framer::new(),
            id: 0,
            poisoned: false,
        },
        profile: dir.clone(),
        pid,
        birth: process_birth,
        image_hash: "offline-own-rust-child".into(),
        context: None,
        target: None,
        version: json!({}),
        close_started: false,
        content_input_uncertain: false,
        signal: SignalState::default(),
        user_uid: uid,
        exit: None,
    };
    let started = Instant::now();
    let first = owned.close();
    let elapsed = started.elapsed();
    let second = owned.close();
    server.join().unwrap();
    let done = dir.join("done").exists();
    if first["process_exit_observed"] == true {
        fs::remove_dir_all(&dir).unwrap();
    }
    assert_eq!(first["code"], 0, "{first}");
    assert_eq!(first["signal"], Value::Null);
    assert!(done, "the exact child test must run its delayed exit body");
    assert!(elapsed >= Duration::from_millis(400));
    assert_eq!(first["forced_own_child_kill_attempted"], false);
    assert_eq!(first["protocol_close"]["browser_close_ack"], true);
    assert_eq!(first["protocol_close"]["attempted_this_call"], true);
    assert_eq!(second["protocol_close"]["attempted_this_call"], false);
    assert_eq!(second["graceful_wait_budget_ms"], 0);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}
