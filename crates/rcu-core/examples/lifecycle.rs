//! Synthetic engine lifecycle and actual zero-target MCP process measurements.
//! No desktop calls. Run the release build, not a debug build.
use rcu_core::Verification;
use rcu_core::{Engine, Intent, Locator, Predicate, fixture::Fixture};
use serde_json::{Value, json};
use std::{
    io::{BufRead, BufReader, Write},
    path::Path,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

#[cfg(target_os = "macos")]
fn metrics(pid: u32) -> Value {
    unsafe {
        let mut info = std::mem::MaybeUninit::<libc::proc_taskinfo>::zeroed();
        let n = libc::proc_pidinfo(
            pid as i32,
            libc::PROC_PIDTASKINFO,
            0,
            info.as_mut_ptr().cast(),
            std::mem::size_of::<libc::proc_taskinfo>() as i32,
        );
        if n != std::mem::size_of::<libc::proc_taskinfo>() as i32 {
            return json!({"unavailable":true});
        }
        let info = info.assume_init();
        let fd_bytes = libc::proc_pidinfo(
            pid as i32,
            libc::PROC_PIDLISTFDS,
            0,
            std::ptr::null_mut(),
            0,
        );
        // The null-buffer query includes spare capacity; count only returned records.
        let fds = if fd_bytes > 0 && fd_bytes < 1_048_576 {
            let mut buffer = vec![0u8; fd_bytes as usize];
            let actual = libc::proc_pidinfo(
                pid as i32,
                libc::PROC_PIDLISTFDS,
                0,
                buffer.as_mut_ptr().cast(),
                fd_bytes,
            );
            if actual >= 0 {
                Some(actual as usize / std::mem::size_of::<libc::proc_fdinfo>())
            } else {
                None
            }
        } else {
            None
        };
        json!({"rss_bytes":info.pti_resident_size,"threads":info.pti_threadnum,"file_descriptors":fds})
    }
}
#[cfg(not(target_os = "macos"))]
fn metrics(_pid: u32) -> Value {
    json!({"unavailable":true})
}
#[cfg(unix)]
fn cpu() -> Option<f64> {
    unsafe {
        let mut r = std::mem::MaybeUninit::<libc::rusage>::zeroed();
        assert_eq!(libc::getrusage(libc::RUSAGE_SELF, r.as_mut_ptr()), 0);
        let r = r.assume_init();
        Some(
            (r.ru_utime.tv_sec + r.ru_stime.tv_sec) as f64
                + (r.ru_utime.tv_usec + r.ru_stime.tv_usec) as f64 / 1e6,
        )
    }
}
#[cfg(not(unix))]
fn cpu() -> Option<f64> {
    None
}
fn stats(mut samples: Vec<f64>) -> Value {
    samples.sort_by(f64::total_cmp);
    let n = samples.len();
    json!({"count":n,"p50_ms":samples[n/2],"p95_ms":samples[((n as f64*0.95).ceil() as usize).saturating_sub(1)],"max_ms":samples[n-1]})
}
fn write(input: &mut impl Write, v: Value) {
    writeln!(input, "{v}").unwrap();
    input.flush().unwrap();
}
fn read(output: &mut impl BufRead) -> Value {
    let mut line = String::new();
    assert!(output.read_line(&mut line).unwrap() > 0);
    serde_json::from_str(&line).unwrap()
}
fn process_bench(binary: &Path) -> Value {
    let state = tempfile::tempdir().unwrap();
    let mut starts = Vec::new();
    let mut discovery = Vec::new();
    let mut pid_info = Value::Null;
    let mut idle = Value::Null;
    for i in 0..10 {
        let start = Instant::now();
        let mut child = Command::new(binary)
            .arg("--state-dir")
            .arg(state.path())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let mut input = child.stdin.take().unwrap();
        let mut output = BufReader::new(child.stdout.take().unwrap());
        write(
            &mut input,
            json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"offline-benchmark","version":"1"}}}),
        );
        let initialized = read(&mut output);
        assert!(initialized["result"].is_object());
        starts.push(start.elapsed().as_secs_f64() * 1000.0);
        write(
            &mut input,
            json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
        );
        let start = Instant::now();
        write(
            &mut input,
            json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}),
        );
        let list = read(&mut output);
        assert_eq!(list["result"]["tools"].as_array().unwrap().len(), 14);
        discovery.push(start.elapsed().as_secs_f64() * 1000.0);
        if i == 9 {
            pid_info = metrics(child.id());
            let start = Instant::now();
            let before = child_cpu(child.id());
            std::thread::sleep(Duration::from_secs(2));
            let after = child_cpu(child.id());
            let elapsed = start.elapsed().as_secs_f64();
            idle = json!({"wall_seconds":elapsed,"cpu_seconds":before.zip(after).map(|(a,b)|b-a),"one_core_percent":before.zip(after).map(|(a,b)|(b-a)/elapsed*100.0),"after":metrics(child.id())});
        }
        drop(input);
        assert!(child.wait().unwrap().success());
    }
    json!({"process_start_to_initialize":stats(starts),"tools_list":stats(discovery),"initialized_process":pid_info,"idle":idle,"samples":10,"cache_state":"fresh process; existing OS filesystem/page cache, not cold boot"})
}
#[cfg(target_os = "macos")]
fn child_cpu(pid: u32) -> Option<f64> {
    unsafe {
        let mut r = std::mem::MaybeUninit::<libc::rusage_info_v2>::zeroed();
        if libc::proc_pid_rusage(pid as i32, libc::RUSAGE_INFO_V2, r.as_mut_ptr().cast()) != 0 {
            return None;
        }
        let r = r.assume_init();
        Some((r.ri_user_time + r.ri_system_time) as f64 / 1e9)
    }
}
#[cfg(not(target_os = "macos"))]
fn child_cpu(_pid: u32) -> Option<f64> {
    None
}
fn main() {
    let binary = std::env::args()
        .nth(1)
        .expect("absolute release rcu-mcp path required");
    let process = process_bench(Path::new(&binary));
    let state = tempfile::tempdir().unwrap();
    let mut engine = Engine::new(state.path().into()).unwrap();
    let app = Fixture::default();
    engine
        .register("fixture".into(), Box::new(app.clone()))
        .unwrap();
    let mut checkpoints = vec![
        json!({"iteration":0,"process":metrics(std::process::id()),"snapshots":engine.snapshot_count(),"leases":engine.leases.active()}),
    ];
    let mut observe = Vec::new();
    let mut act = Vec::new();
    let mut verify = Vec::new();
    let count = 10_000;
    let start = Instant::now();
    let cpu_start = cpu();
    for i in 0..count {
        let now = Instant::now();
        let before = engine.observe("fixture").unwrap();
        observe.push(now.elapsed().as_secs_f64() * 1000.0);
        let r = engine
            .resolve(
                "fixture",
                &before.frame,
                Locator::Semantic {
                    name: "Input".into(),
                },
            )
            .unwrap();
        let lease = engine.claim("fixture", 60000).unwrap();
        let request = format!("lifecycle-{i}");
        let expected = format!("iteration {i}: 中文🙂\n  whitespace preserved");
        let now = Instant::now();
        let receipt = engine
            .act(
                "fixture",
                &r.reference,
                &lease,
                &request,
                Intent::Text,
                Some(expected.clone()),
            )
            .unwrap();
        act.push(now.elapsed().as_secs_f64() * 1000.0);
        assert!(!receipt.goal_verified);
        assert_eq!(receipt.dispatch, "returned");
        let after = engine.observe("fixture").unwrap();
        let now = Instant::now();
        let result = engine
            .verify(Verification {
                target: "fixture",
                frame: &after.frame,
                element: Some("Input".into()),
                request: Some(request),
                predicate: Predicate::ElementValue,
                expected: &expected,
                timeout_ms: 0,
                stable_samples: 1,
            })
            .unwrap();
        verify.push(now.elapsed().as_secs_f64() * 1000.0);
        assert_eq!(result["goal_verified"], true);
        engine.release(&lease).unwrap();
        if (i + 1) % 1000 == 0 {
            checkpoints.push(json!({"iteration":i+1,"process":metrics(std::process::id()),"snapshots":engine.snapshot_count(),"artifact_bytes":engine.artifact_bytes(),"leases":engine.leases.active()}));
            eprintln!("completed {} / {count} synthetic cycles", i + 1);
        }
    }
    let elapsed = start.elapsed().as_secs_f64();
    let cpu_seconds = cpu_start.zip(cpu()).map(|(start, end)| end - start);
    let (receipts, bytes) = std::fs::read_dir(state.path().join("receipts"))
        .unwrap()
        .fold((0usize, 0u64), |(count, bytes), entry| {
            let entry = entry.unwrap();
            (count + 1, bytes + entry.metadata().unwrap().len())
        });
    assert_eq!(app.0.lock().unwrap().calls, count);
    assert_eq!(receipts, count as usize);
    assert_eq!(engine.leases.active(), 0);
    assert_eq!(engine.snapshot_count(), 64);
    drop(engine);
    std::thread::sleep(Duration::from_millis(50));
    println!("{}",serde_json::to_string_pretty(&json!({"schema":1,"backend":"synthetic-fixture","native_capture_performed":false,"native_input_performed":false,"native_capture_latency_ms":null,"native_verify_latency_ms":null,"comparative_speedup_claim":null,"build":"release","iterations":count,"elapsed_seconds":elapsed,"cpu_seconds":cpu_seconds,"one_core_percent":cpu_seconds.map(|s|s/elapsed*100.0),"actual_mcp_process":process,"fixture_observe":stats(observe),"fixture_action_with_durable_fsync":stats(act),"fixture_verify_independent_model_single_sample":stats(verify),"checkpoints":checkpoints,"after_engine_drop":metrics(std::process::id()),"persistent_journal":{"files":receipts,"bytes":bytes,"retention":"not pruned; deliberate at-most-once records; no production quota/compaction yet"}})).unwrap());
}
