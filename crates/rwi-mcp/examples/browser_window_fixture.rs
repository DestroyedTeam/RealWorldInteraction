//! Disposable, opt-in browser reception oracle. Never a GUI input backend.
//! The page observes DOM events; all test input must arrive through native RWI.
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, VecDeque},
    fs::{self, OpenOptions},
    io::{self, Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    path::PathBuf,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const HEADER_LIMIT: usize = 8 * 1024;
const BODY_LIMIT: usize = 4 * 1024;
const EVENTS_PER_WINDOW: usize = 512;
const REQUEST_LIMIT: usize = 10_000;
const WINDOW_IDS: [&str; 3] = ["a", "b", "c"];
const EVENT_TYPES: [&str; 14] = [
    "page_loaded",
    "heartbeat",
    "window_focus",
    "window_blur",
    "visibility",
    "click",
    "pointermove",
    "pointerenter",
    "pointerleave",
    "keydown",
    "input",
    "scroll",
    "wheel",
    "pagehide",
];

fn invalid(message: &str) -> io::Error {
    io::Error::other(message.to_owned())
}
fn unix_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

struct Config {
    port: u16,
    output: Option<PathBuf>,
    run: String,
    token: Option<String>,
    lifetime: Duration,
    check_only: bool,
}
impl Config {
    fn parse(args: impl IntoIterator<Item = String>) -> io::Result<Self> {
        let mut fields = BTreeMap::new();
        let mut check_only = false;
        for arg in args {
            if arg == "--check-only" && !check_only {
                check_only = true;
                continue;
            }
            let (key, value) = arg
                .split_once('=')
                .ok_or_else(|| invalid("expected --name=value or --check-only"))?;
            if !matches!(
                key,
                "--port" | "--output" | "--run" | "--token" | "--lifetime-seconds"
            ) || fields.insert(key.to_owned(), value.to_owned()).is_some()
            {
                return Err(invalid("unknown or duplicated argument"));
            }
        }
        let run = fields
            .remove("--run")
            .unwrap_or_else(|| "rwi-browser".into());
        if run.is_empty()
            || run.len() > 64
            || !run.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
        {
            return Err(invalid(
                "run must be a 1..64 ASCII alphanumeric/hyphen slug",
            ));
        }
        let output = fields.remove("--output").map(PathBuf::from);
        let token = fields.remove("--token");
        if token.as_ref().is_some_and(|value| {
            value.is_empty()
                || value.len() > 128
                || !value
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-')
        }) {
            return Err(invalid(
                "token must be a 1..128 ASCII alphanumeric/hyphen slug",
            ));
        }
        if output.as_ref().is_some_and(|path| !path.is_absolute()) {
            return Err(invalid("output must be an absolute new evidence directory"));
        }
        if output.is_none() && !check_only {
            return Err(invalid("missing --output"));
        }
        let port = fields
            .remove("--port")
            .unwrap_or_else(|| "0".into())
            .parse::<u16>()
            .map_err(|_| invalid("invalid port"))?;
        let seconds = fields
            .remove("--lifetime-seconds")
            .unwrap_or_else(|| "1800".into())
            .parse::<u64>()
            .map_err(|_| invalid("invalid lifetime"))?;
        if !(1..=3600).contains(&seconds) {
            return Err(invalid("lifetime must be 1..3600 seconds"));
        }
        Ok(Self {
            port,
            output,
            run,
            token,
            lifetime: Duration::from_secs(seconds),
            check_only,
        })
    }
}

struct Request {
    method: String,
    path: String,
    host: String,
    origin: Option<String>,
    body: Vec<u8>,
}
fn read_request(stream: &mut TcpStream) -> io::Result<Request> {
    // BSD accept may inherit O_NONBLOCK; socket deadlines require blocking I/O.
    stream.set_nonblocking(false)?;
    // A peer cannot keep the server alive indefinitely by trickling one byte
    // before each socket timeout. The whole request shares an absolute budget.
    let deadline = Instant::now() + Duration::from_secs(2);
    stream.set_write_timeout(Some(Duration::from_millis(500)))?;
    let mut bytes = Vec::with_capacity(1024);
    let header_end = loop {
        let mut byte = [0];
        remaining_read_timeout(stream, deadline)?;
        stream.read_exact(&mut byte)?;
        bytes.push(byte[0]);
        if bytes.ends_with(b"\r\n\r\n") {
            break bytes.len();
        }
        if bytes.len() >= HEADER_LIMIT {
            return Err(invalid("HTTP header budget"));
        }
    };
    let header = std::str::from_utf8(&bytes).map_err(|_| invalid("invalid HTTP header"))?;
    let mut lines = header.split("\r\n");
    let mut first = lines.next().unwrap_or_default().split_whitespace();
    let method = first.next().unwrap_or_default().to_owned();
    let path = first.next().unwrap_or_default().to_owned();
    if first.next() != Some("HTTP/1.1") || first.next().is_some() {
        return Err(invalid("HTTP/1.1 request required"));
    }
    let mut headers = BTreeMap::new();
    for line in lines.filter(|line| !line.is_empty()) {
        let (name, value) = line
            .split_once(':')
            .ok_or_else(|| invalid("invalid header"))?;
        let name = name.to_ascii_lowercase();
        if headers.insert(name, value.trim().to_owned()).is_some() {
            return Err(invalid("duplicate HTTP header"));
        }
    }
    if headers.contains_key("transfer-encoding") {
        return Err(invalid("chunked requests are not accepted"));
    }
    let length = headers.get("content-length").map_or(Ok(0), |value| {
        value
            .parse::<usize>()
            .map_err(|_| invalid("invalid content length"))
    })?;
    if length > BODY_LIMIT || (method != "POST" && length != 0) {
        return Err(invalid("HTTP body budget or method"));
    }
    bytes.resize(header_end + length, 0);
    let mut read = header_end;
    while read < bytes.len() {
        remaining_read_timeout(stream, deadline)?;
        let count = stream.read(&mut bytes[read..])?;
        if count == 0 {
            return Err(invalid("truncated HTTP body"));
        }
        read += count;
    }
    Ok(Request {
        method,
        path,
        host: headers.remove("host").unwrap_or_default(),
        origin: headers.remove("origin"),
        body: bytes[header_end..].to_vec(),
    })
}
fn remaining_read_timeout(stream: &TcpStream, deadline: Instant) -> io::Result<()> {
    let remaining = deadline
        .checked_duration_since(Instant::now())
        .filter(|duration| !duration.is_zero())
        .ok_or_else(|| invalid("HTTP request deadline"))?;
    stream.set_read_timeout(Some(remaining.min(Duration::from_millis(500))))
}
fn respond(stream: &mut TcpStream, status: &str, mime: &str, body: &[u8]) -> io::Result<()> {
    write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Type: {mime}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\nX-Content-Type-Options: nosniff\r\nContent-Security-Policy: default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; connect-src 'self'; form-action 'none'; frame-ancestors 'none'\r\n\r\n",
        body.len()
    )?;
    stream.write_all(body)
}

#[derive(Default)]
struct WindowState {
    received: u64,
    dropped: u64,
    events: VecDeque<Value>,
}
struct Oracle {
    run: String,
    started_ms: u128,
    windows: BTreeMap<String, WindowState>,
}
impl Oracle {
    fn new(run: &str) -> Self {
        Self {
            run: run.into(),
            started_ms: unix_ms(),
            windows: WINDOW_IDS
                .into_iter()
                .map(|id| (id.into(), WindowState::default()))
                .collect(),
        }
    }
    fn accept(&mut self, id: &str, body: &[u8]) -> io::Result<Value> {
        if body.len() > BODY_LIMIT {
            return Err(invalid("event body budget"));
        }
        let event: Value = serde_json::from_slice(body)?;
        if !event.is_object()
            || event["window"] != id
            || !event["type"]
                .as_str()
                .is_some_and(|kind| EVENT_TYPES.contains(&kind))
        {
            return Err(invalid("known page/window event required; no commands"));
        }
        let window = self
            .windows
            .get_mut(id)
            .ok_or_else(|| invalid("unknown window"))?;
        window.received += 1;
        let receipt = json!({"received_unix_ms":unix_ms(),"server_sequence":window.received,"page_event":event});
        if window.events.len() == EVENTS_PER_WINDOW {
            window.events.pop_front();
            window.dropped += 1;
        }
        window.events.push_back(receipt.clone());
        Ok(receipt)
    }
    fn state(&self, status: &str) -> Value {
        let windows: BTreeMap<_, _> = self.windows.iter().map(|(id, window)| {
            (id, json!({"received":window.received,"dropped":window.dropped,"events":window.events}))
        }).collect();
        json!({"schema_version":1,"run":self.run,"started_unix_ms":self.started_ms,
            "sampled_unix_ms":unix_ms(),"status":status,"windows":windows,
            "input_authority":false,"input_sent_by_server":false,
            "assurance":"Independent client-reported DOM reception oracle, not a native dispatch/physical-origin attestation.",
            "limits":{"windows":3,"events_per_window":EVENTS_PER_WINDOW,"event_bytes":BODY_LIMIT}})
    }
}

fn page(run: &str, token: &str, id: &str) -> String {
    let label = id.to_ascii_uppercase();
    PAGE.replace("__RUN__", run)
        .replace("__TOKEN__", token)
        .replace("__ID__", id)
        .replace("__LABEL__", &label)
}
const PAGE: &str = include_str!("browser_window_fixture/page.html");

fn write_state(path: &std::path::Path, value: &Value) -> io::Result<()> {
    let temporary = path.with_extension("json.tmp");
    fs::write(&temporary, serde_json::to_vec_pretty(value)?)?;
    fs::rename(temporary, path)
}
fn handle(
    stream: &mut TcpStream,
    address: SocketAddr,
    token: &str,
    oracle: &mut Oracle,
    log: &mut fs::File,
    state_path: &std::path::Path,
) -> io::Result<()> {
    let request = read_request(stream)?;
    let host = address.to_string();
    let origin = format!("http://{host}");
    if request.host != host
        || request
            .origin
            .as_ref()
            .is_some_and(|value| value != &origin)
    {
        return respond(
            stream,
            "403 Forbidden",
            "text/plain",
            b"loopback same-origin only",
        );
    }
    let prefix = format!("/{token}/");
    let Some(route) = request.path.strip_prefix(&prefix) else {
        return respond(
            stream,
            "404 Not Found",
            "text/plain",
            b"unknown fixture route",
        );
    };
    if request.method == "GET" && route == "state" {
        return respond(
            stream,
            "200 OK",
            "application/json",
            &serde_json::to_vec(&oracle.state("running"))?,
        );
    }
    for id in WINDOW_IDS {
        if request.method == "GET" && route == format!("{id}/") {
            return respond(
                stream,
                "200 OK",
                "text/html; charset=utf-8",
                page(&oracle.run, token, id).as_bytes(),
            );
        }
        if request.method == "POST" && route == format!("{id}/events") {
            let receipt = oracle.accept(id, &request.body)?;
            writeln!(log, "{receipt}")?;
            log.flush()?;
            write_state(state_path, &oracle.state("running"))?;
            return respond(
                stream,
                "200 OK",
                "application/json",
                b"{\"recorded\":true,\"input_sent\":false}",
            );
        }
    }
    respond(
        stream,
        "404 Not Found",
        "text/plain",
        b"unknown fixture route",
    )
}

fn run() -> io::Result<()> {
    let config = Config::parse(std::env::args().skip(1))?;
    if config.check_only {
        println!(
            "{}",
            json!({"check_only":true,"run":config.run,"output":config.output,
            "gui_created":false,"listener_started":false,"input_sent":false})
        );
        return Ok(());
    }
    let output = config
        .output
        .as_ref()
        .ok_or_else(|| invalid("missing --output"))?;
    fs::create_dir(output)?;
    let listener = TcpListener::bind(("127.0.0.1", config.port))?;
    listener.set_nonblocking(true)?;
    let address = listener.local_addr()?;
    let token_source = if config.token.is_some() {
        "provided_for_allocated_fixture"
    } else {
        "generated"
    };
    let token = config
        .token
        .unwrap_or_else(|| format!("{}-{}", config.run, uuid::Uuid::new_v4().simple()));
    let base = format!("http://{address}/{token}/");
    let ready = json!({"schema_version":1,"port":address.port(),"run":config.run,"token":token,"token_source":token_source,
        "base_url":base,"pages":{"a":format!("{base}a/"),"b":format!("{base}b/"),"c":format!("{base}c/")},
        "state_url":format!("{base}state"),"output":config.output,"lifetime_seconds":config.lifetime.as_secs(),
        "input_authority":false,"gui_created":false,"browser_launched":false});
    fs::write(
        output.join("READY.json"),
        serde_json::to_vec_pretty(&ready)?,
    )?;
    let state_path = output.join("STATE.json");
    let mut log = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output.join("events.jsonl"))?;
    let mut oracle = Oracle::new(&config.run);
    write_state(&state_path, &oracle.state("running"))?;
    println!("{ready}");
    io::stdout().flush()?;
    let start = Instant::now();
    let mut requests = 0;
    while start.elapsed() < config.lifetime && requests < REQUEST_LIMIT {
        match listener.accept() {
            Ok((mut stream, peer)) => {
                requests += 1;
                if !peer.ip().is_loopback() {
                    continue;
                }
                if let Err(error) = handle(
                    &mut stream,
                    address,
                    &token,
                    &mut oracle,
                    &mut log,
                    &state_path,
                ) {
                    eprintln!("request {requests}: {error}");
                    let _ = respond(
                        &mut stream,
                        "400 Bad Request",
                        "text/plain",
                        b"invalid or over-budget oracle observation",
                    );
                }
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(10))
            }
            Err(error) => return Err(error),
        }
    }
    let reason = if requests == REQUEST_LIMIT {
        "request_limit"
    } else {
        "lifetime_expired"
    };
    write_state(&state_path, &oracle.state(reason))?;
    println!(
        "{}",
        json!({"status":reason,"requests":requests,"input_sent":false,"browser_closed_by_server":false})
    );
    Ok(())
}
fn main() -> io::Result<()> {
    run()
}

#[cfg(test)]
#[path = "browser_window_fixture/tests.rs"]
mod tests;
