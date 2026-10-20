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
fn empty_eof_is_not_a_request_and_diagnostics_do_not_copy_a_route() {
    let error = request(b"").err().unwrap();
    assert!(is_no_request(&error));
    assert_eq!(error.kind(), io::ErrorKind::UnexpectedEof);
    let diagnostic = error.to_string();
    assert!(diagnostic.contains("phase=before_request_line"));
    assert!(diagnostic.contains("header_bytes=0"));
    assert!(diagnostic.contains("outcome=closed_without_response"));
    assert!(!diagnostic.contains('/'));
    assert!(!diagnostic.contains("token"));
}
#[test]
fn idle_empty_connection_is_bounded_and_does_not_become_a_request() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let _client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
    let (mut stream, _) = listener.accept().unwrap();
    let started = Instant::now();
    let error = read_request(&mut stream).err().unwrap();
    assert!(is_no_request(&error));
    assert!(matches!(
        error.kind(),
        io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
    ));
    assert!(started.elapsed() < Duration::from_secs(2));
}
#[test]
fn partial_header_eof_and_complete_malformed_request_still_require_errors() {
    for raw in [
        b"G".as_slice(),
        b"GET /secret-token/b/ HTTP/1.0\r\nHost: 127.0.0.1:1234\r\n\r\n".as_slice(),
    ] {
        let error = request(raw).err().unwrap();
        assert!(!is_no_request(&error));
        assert!(!error.to_string().contains("secret-token"));
    }
}
#[test]
fn partial_header_idle_keeps_its_error_and_original_read_budget() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let mut client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
    client.write_all(b"G").unwrap();
    let (mut stream, _) = listener.accept().unwrap();
    let started = Instant::now();
    let error = read_request(&mut stream).err().unwrap();
    assert!(!is_no_request(&error));
    assert!(matches!(
        error.kind(),
        io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
    ));
    assert!(started.elapsed() < Duration::from_secs(2));
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

#[test]
fn r95_document_mouse_capture_is_readonly_and_separate_from_hover_family() {
    let html = page("r95-test", "test-token", "a");
    assert_eq!(
        html.matches(
            "document.addEventListener(kind,observeDocumentMouse,{capture:true,passive:true})"
        )
        .count(),
        1
    );
    for required in [
        "['mousedown','mouseup','mousemove']",
        "documentMouseCounts={mousedown:0,mouseup:0,mousemove:0}",
        "emit(event.type,event,'document_mouse_capture')",
        "'hover_pointer':'page_or_control'",
        "observerFamily!=='document_mouse_capture'",
        "documentMouseSequence++",
        "nativeEventType:event?event.type:null",
        "getBoundingClientRect()",
        "nearestButtonId",
    ] {
        assert!(html.contains(required), "{required}");
    }
    for forbidden in [
        "dispatchEvent",
        ".click(",
        ".focus(",
        "Input.dispatch",
        "preventDefault(",
        "stopPropagation(",
    ] {
        assert!(!html.contains(forbidden), "{forbidden}");
    }
    assert_eq!(
        html.matches("hover.addEventListener('pointermove'").count(),
        1
    );
    assert_eq!(
        html.matches("hover.addEventListener('pointerenter'")
            .count(),
        1
    );
    assert_eq!(
        html.matches("hover.addEventListener('pointerleave'")
            .count(),
        1
    );
}

#[test]
fn r95_document_mouse_reception_types_are_raw_and_counted_separately() {
    let mut oracle = Oracle::new("r95-test");
    for (sequence, kind) in ["mousedown", "mouseup", "mousemove"]
        .into_iter()
        .enumerate()
    {
        let event = json!({"window":"a","type":kind,"nativeEventType":kind,
            "observerFamily":"document_mouse_capture","documentMouseSequence":sequence+1,
            "isTrusted":false,"counter":0,"clientX":100,"clientY":80});
        let receipt = oracle
            .accept("a", &serde_json::to_vec(&event).unwrap())
            .unwrap();
        assert_eq!(receipt["page_event"], event);
        assert_eq!(oracle.windows["a"].event_type_counts[kind], 1);
    }
    let hover = json!({"window":"a","type":"pointermove","observerFamily":"hover_pointer"});
    oracle
        .accept("a", &serde_json::to_vec(&hover).unwrap())
        .unwrap();
    assert_eq!(oracle.windows["a"].event_type_counts["mousemove"], 1);
    assert_eq!(oracle.windows["a"].event_type_counts["pointermove"], 1);
    assert_eq!(oracle.windows["a"].received, 4);
    assert!(oracle.windows["b"].event_type_counts.is_empty());
    assert_eq!(
        oracle.state("test")["windows"]["a"]["event_type_counts"]["mousedown"],
        1
    );
    assert_eq!(oracle.state("test")["input_sent_by_server"], false);
}

#[test]
fn r95_document_mouse_counts_survive_bounded_ring_eviction() {
    let mut oracle = Oracle::new("r95-test");
    oracle
        .accept(
            "a",
            br#"{"window":"a","type":"mousedown","observerFamily":"document_mouse_capture"}"#,
        )
        .unwrap();
    for _ in 0..EVENTS_PER_WINDOW {
        oracle
            .accept("a", br#"{"window":"a","type":"heartbeat"}"#)
            .unwrap();
    }
    assert_eq!(oracle.windows["a"].events.len(), EVENTS_PER_WINDOW);
    assert_eq!(oracle.windows["a"].dropped, 1);
    assert_eq!(oracle.windows["a"].event_type_counts["mousedown"], 1);
    assert_eq!(
        oracle.windows["a"].event_type_counts["heartbeat"],
        EVENTS_PER_WINDOW as u64
    );
}

#[test]
fn receiver_observer_page_fields_and_new_listeners_are_passive() {
    let html = page("observer-test", "test-token", "a");
    for field in ["keyCode", "charCode", "isComposing"] {
        assert!(html.contains(&format!(
            "{field}:event&&'{field}'in event?event.{field}:null"
        )));
    }
    assert_eq!(html.matches("document.addEventListener(kind,event=>emit(kind,event,'receiving_chain'),{capture:true,passive:true})").count(), 1);
    for forbidden in [
        "preventDefault",
        "stopPropagation",
        "dispatchEvent",
        ".focus(",
        ".click(",
    ] {
        assert!(!html.contains(forbidden));
    }
    assert_eq!(HEADER_LIMIT, 8192);
    assert_eq!(BODY_LIMIT, 4096);
    assert_eq!(EVENTS_PER_WINDOW, 512);
    assert_eq!(REQUEST_LIMIT, 10_000);
}

#[test]
fn receiver_observer_text_chain_receipts_preserve_raw_fields_and_isolation() {
    let mut oracle = Oracle::new("observer-test");
    for (sequence, kind) in [
        "keypress",
        "beforeinput",
        "compositionstart",
        "compositionupdate",
        "compositionend",
    ]
    .into_iter()
    .enumerate()
    {
        let event = json!({"window":"a","type":kind,"nativeEventType":kind,"observerFamily":"receiving_chain","clientSequence":sequence+1,"keyCode":229,"charCode":0,"isComposing":false,"isTrusted":false,"data":"🙂"});
        let receipt = oracle
            .accept("a", &serde_json::to_vec(&event).unwrap())
            .unwrap();
        assert_eq!(receipt["page_event"], event);
        assert_eq!(oracle.windows["a"].event_type_counts[kind], 1);
    }
    assert_eq!(oracle.windows["a"].received, 5);
    assert_eq!(oracle.windows["b"].received, 0);
    assert_eq!(oracle.windows["c"].received, 0);
}

#[test]
fn receiver_observer_unknown_command_and_byte_budget_still_fail_closed() {
    let mut oracle = Oracle::new("observer-test");
    for event in [
        json!({"window":"a","type":"composition_commit_command"}),
        json!({"window":"b","type":"beforeinput"}),
        json!({"window":"a","type":"beforeinput","data":"🙂".repeat(BODY_LIMIT)}),
    ] {
        assert!(
            oracle
                .accept("a", &serde_json::to_vec(&event).unwrap())
                .is_err()
        );
    }
    assert_eq!(oracle.windows["a"].received, 0);
}
