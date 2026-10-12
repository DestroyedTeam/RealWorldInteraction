use super::*;
fn request(raw: &[u8]) -> io::Result<Request> {
    let listener = TcpListener::bind(("127.0.0.1", 0))?;
    let mut client = TcpStream::connect(listener.local_addr()?)?;
    client.write_all(raw)?;
    client.shutdown(std::net::Shutdown::Write)?;
    let (mut stream, _) = listener.accept()?;
    read_request(&mut stream)
}
fn config(extra: &[&str]) -> io::Result<Config> {
    Config::parse(
        [
            "--run=r91-test".to_owned(),
            format!(
                "--output={}",
                std::env::temp_dir().join("rwi-fixture-new").display()
            ),
            "--port=0".to_owned(),
        ]
        .into_iter()
        .chain(extra.iter().map(|arg| (*arg).to_owned())),
    )
}
#[test]
fn check_only_keeps_allocation_and_does_not_need_output_to_exist() {
    let config = config(&["--check-only", "--lifetime-seconds=1800"]).unwrap();
    assert!(config.check_only);
    assert_eq!(config.port, 0);
    assert_eq!(config.lifetime, Duration::from_secs(1800));
    let no_output = Config::parse(["--check-only".into()]).unwrap();
    assert!(no_output.check_only);
    assert!(no_output.output.is_none());
}
#[test]
fn allocation_arguments_are_explicit_bounded_and_not_duplicated() {
    for extra in [
        vec!["--run=second"],
        vec!["--lifetime-seconds=3601"],
        vec!["--check-only", "--check-only"],
        vec!["--browser-command=click"],
    ] {
        assert!(config(&extra).is_err());
    }
    assert!(Config::parse(["--output=relative".into()]).is_err());
    assert!(Config::parse(["--output=/tmp/new".into(), "--run=<script>".into()]).is_err());
}
#[test]
fn http_observation_parser_preserves_body_and_refuses_ambiguous_framing() {
    let parsed = request(b"POST /token/a/events HTTP/1.1\r\nHost: 127.0.0.1:1234\r\nOrigin: http://127.0.0.1:1234\r\nContent-Length: 2\r\n\r\n{}").unwrap();
    assert_eq!(parsed.path, "/token/a/events");
    assert_eq!(parsed.body, b"{}");
    assert_eq!(parsed.origin.as_deref(), Some("http://127.0.0.1:1234"));
    for raw in [
        b"POST /token/a/events HTTP/1.1\r\nContent-Length: 0\r\nContent-Length: 2\r\n\r\n"
            .as_slice(),
        b"POST /token/a/events HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n".as_slice(),
        b"POST /token/a/events HTTP/1.1\r\nContent-Length: 4097\r\n\r\n".as_slice(),
        b"GET /token/state HTTP/1.1\r\nContent-Length: 2\r\n\r\n{}".as_slice(),
        b"POST /token/a/events HTTP/1.1\r\nContent-Length: 3\r\n\r\n{}".as_slice(),
    ] {
        assert!(request(raw).is_err());
    }
}
#[test]
fn page_is_only_a_received_event_oracle_not_an_input_bridge() {
    let html = page("r91-test", "test-token", "a");
    for forbidden in [
        "dispatchEvent",
        ".click(",
        ".focus(",
        "Input.dispatch",
        "eval(",
        "new MouseEvent",
        "new KeyboardEvent",
        "navigator.clipboard",
        "window.open(",
        "keepalive:true",
    ] {
        assert!(!html.contains(forbidden), "{forbidden}");
    }
    for required in [
        "addEventListener('click'",
        "event.isTrusted",
        "document.hasFocus()",
        "visibilityState",
        "scrollTop",
        "inputValue",
        "clientSequence",
        "Window A",
        "/test-token/a/events",
        "keepalive:type==='pagehide'",
        "await res.text()",
        "if(!res.ok)",
        "if(!oracleOffline)",
        "oracleOffline=true",
    ] {
        assert!(html.contains(required), "{required}");
    }
}
#[test]
fn window_event_receipts_are_raw_isolated_and_bounded() {
    let mut oracle = Oracle::new("run");
    let event = json!({"window":"a","type":"click","isTrusted":true,"counter":7,"inputValue":"α"});
    let receipt = oracle
        .accept("a", &serde_json::to_vec(&event).unwrap())
        .unwrap();
    assert_eq!(receipt["page_event"], event);
    assert_eq!(oracle.windows["a"].received, 1);
    assert_eq!(oracle.windows["b"].received, 0);
    for _ in 0..EVENTS_PER_WINDOW {
        oracle
            .accept("a", &serde_json::to_vec(&event).unwrap())
            .unwrap();
    }
    assert_eq!(oracle.windows["a"].events.len(), EVENTS_PER_WINDOW);
    assert_eq!(oracle.windows["a"].dropped, 1);
}
#[test]
fn command_unknown_window_invalid_event_or_over_budget_is_rejected() {
    let mut oracle = Oracle::new("run");
    for event in [
        json!({"window":"a","type":"click_command"}),
        json!({"window":"b","type":"click"}),
        json!(["click"]),
    ] {
        assert!(
            oracle
                .accept("a", &serde_json::to_vec(&event).unwrap())
                .is_err()
        );
    }
    assert!(
        oracle
            .accept("d", br#"{"window":"d","type":"click"}"#)
            .is_err()
    );
    assert!(oracle.accept("a", &vec![b' '; BODY_LIMIT + 1]).is_err());
    assert_eq!(oracle.windows["a"].received, 0);
}

const PACKET: &[u8] = b"POST /token/a/events HTTP/1.1\r\nHost: 127.0.0.1:1234\r\nOrigin: http://127.0.0.1:1234\r\nContent-Length: 2\r\n\r\n{}";

fn fragmented(delay: Duration) -> (io::Result<Request>, Duration) {
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let mut client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
    let (mut stream, _) = listener.accept().unwrap();
    // Force the accepted socket state on every OS, even where accept differs.
    stream.set_nonblocking(true).unwrap();
    let writer = std::thread::spawn(move || {
        for byte in PACKET {
            std::thread::sleep(delay);
            if client.write_all(&[*byte]).is_err() {
                break;
            }
        }
    });
    let started = Instant::now();
    let result = read_request(&mut stream);
    let elapsed = started.elapsed();
    drop(stream);
    writer.join().unwrap();
    (result, elapsed)
}

#[test]
fn inherited_nonblocking_socket_accepts_fragmented_header_and_body() {
    let (result, _) = fragmented(Duration::from_millis(2));
    let parsed = result.unwrap();
    assert_eq!(parsed.method, "POST");
    assert_eq!(parsed.path, "/token/a/events");
    assert_eq!(parsed.host, "127.0.0.1:1234");
    assert_eq!(parsed.body, b"{}");
    assert_eq!(parsed.origin.as_deref(), Some("http://127.0.0.1:1234"));
}

#[test]
fn inherited_nonblocking_socket_trickle_keeps_absolute_budget() {
    let (result, elapsed) = fragmented(Duration::from_millis(100));
    assert!(result.is_err());
    assert!(elapsed >= Duration::from_millis(1800), "{elapsed:?}");
    assert!(elapsed < Duration::from_secs(3), "{elapsed:?}");
}

#[test]
fn allocated_token_is_optional_exact_bounded_and_not_duplicated() {
    assert!(config(&[]).unwrap().token.is_none());
    assert_eq!(
        config(&["--token=original-READY-token"])
            .unwrap()
            .token
            .as_deref(),
        Some("original-READY-token")
    );
    for token in [
        "",
        "space token",
        "bad/token",
        "bad_token",
        "α",
        &"a".repeat(129),
    ] {
        assert!(config(&[&format!("--token={token}")]).is_err());
    }
    assert!(config(&["--token=a", "--token=b"]).is_err());
    assert_eq!(
        config(&[&format!("--token={}", "a".repeat(128))])
            .unwrap()
            .token
            .unwrap()
            .len(),
        128
    );
}
