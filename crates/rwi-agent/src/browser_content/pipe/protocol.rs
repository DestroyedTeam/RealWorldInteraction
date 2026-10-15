//! JSON/NUL framed pipe protocol; bounded frames and exact reply correlation.
use super::{CAP, invalid};
use serde_json::{Value, json};
use std::{
    io::{self, Read, Write},
    os::unix::net::UnixStream,
    time::{Duration, Instant},
};
pub(super) struct Framer {
    buffered: Vec<u8>,
}
impl Framer {
    pub(super) fn new() -> Self {
        Self {
            buffered: Vec::new(),
        }
    }
    pub(super) fn push(&mut self, bytes: &[u8]) -> io::Result<()> {
        if self.buffered.len() + bytes.len() > CAP + 8192 {
            return Err(invalid("browser_pipe_buffer_limit"));
        }
        self.buffered.extend_from_slice(bytes);
        if self
            .buffered
            .iter()
            .position(|b| *b == 0)
            .unwrap_or(self.buffered.len())
            > CAP
        {
            return Err(invalid("browser_pipe_frame_limit"));
        }
        Ok(())
    }
    pub(super) fn next(&mut self) -> io::Result<Option<Value>> {
        let Some(end) = self.buffered.iter().position(|b| *b == 0) else {
            return Ok(None);
        };
        if end == 0 || end > CAP {
            return Err(invalid("browser_pipe_frame_invalid"));
        }
        let value = serde_json::from_slice(&self.buffered[..end])?;
        self.buffered.drain(..=end);
        Ok(Some(value))
    }
}
pub(super) struct Pipe {
    pub(super) tx: UnixStream,
    pub(super) rx: UnixStream,
    pub(super) framer: Framer,
    pub(super) id: u64,
    pub(super) poisoned: bool,
}
impl Pipe {
    pub(super) fn rpc(&mut self, method: &str, params: Value, until: Instant) -> io::Result<Value> {
        if self.poisoned {
            return Err(invalid("browser_pipe_poisoned_no_replay"));
        }
        let id = self
            .id
            .checked_add(1)
            .ok_or_else(|| invalid("browser_pipe_id_exhausted"))?;
        self.id = id;
        let mut raw = serde_json::to_vec(&json!({"id":id,"method":method,"params":params}))?;
        raw.push(0);
        if raw.len() > CAP {
            return Err(invalid("browser_pipe_request_limit"));
        }
        // Poison before first attempted write, including partial write/EOF/lost ACK.
        // Only a correctly bound complete reply clears it. No command is resent.
        self.poisoned = true;
        let mut written = 0;
        while written < raw.len() {
            if Instant::now() >= until {
                return Err(invalid(format!(
                    "browser_pipe_write_timeout_effect_unknown:written={written}"
                )));
            }
            match self.tx.write(&raw[written..]) {
                Ok(0) => return Err(invalid("browser_pipe_write_zero_effect_unknown")),
                Ok(n) => written += n,
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(1))
                }
                Err(e) => return Err(invalid(format!("browser_pipe_write_effect_unknown:{e}"))),
            }
        }
        let mut events = 0;
        loop {
            if Instant::now() >= until {
                return Err(invalid("browser_pipe_reply_timeout_effect_unknown"));
            }
            if let Some(v) = self.framer.next()? {
                if v.get("id").is_some() {
                    if v["id"].as_u64() != Some(id) {
                        return Err(invalid("browser_pipe_reply_identity_mismatch"));
                    }
                    if let Some(e) = v.get("error") {
                        return Err(invalid(format!("browser_protocol_refused:{e}")));
                    }
                    let Some(result) = v.get("result") else {
                        return Err(invalid("browser_pipe_result_missing"));
                    };
                    self.poisoned = false;
                    return Ok(result.clone());
                }
                if !v.get("method").is_some_and(Value::is_string) || events >= 64 {
                    return Err(invalid("browser_pipe_event_limit_or_shape"));
                }
                events += 1;
                continue;
            }
            let mut b = [0u8; 8192];
            match self.rx.read(&mut b) {
                Ok(0) => return Err(invalid("browser_pipe_eof_effect_unknown")),
                Ok(n) => self.framer.push(&b[..n])?,
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(1))
                }
                Err(e) => return Err(invalid(format!("browser_pipe_read_effect_unknown:{e}"))),
            }
        }
    }
}
