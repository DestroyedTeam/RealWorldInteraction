// Adapted unchanged framing algorithm from accepted B1, not its fixture trust/runtime.
pub const ATTACH_CAP: usize = 64 * 1024;
pub const INBOUND: usize = 1024 * 1024;
pub const OUTBOUND: usize = 32 * 1024 * 1024;
use crate::invalid;
use std::io;
use tokio::io::{AsyncBufRead, AsyncBufReadExt};
/// Bound includes CR/LF. Parsing validates, but returned bytes are not normalized.
pub async fn read_frame<R: AsyncBufRead + Unpin>(
    input: &mut R,
    cap: usize,
) -> io::Result<Option<Vec<u8>>> {
    let raw = read_raw_frame(input, cap).await?;
    if let Some(bytes) = &raw {
        let value: serde_json::Value = serde_json::from_slice(bytes)?;
        if !value.is_object() {
            return Err(invalid("jsonrpc_frame_must_be_object"));
        }
    }
    Ok(raw)
}
/// Bounded framing before semantic validation, for retaining an owned worker's
/// invalid stdout as failure evidence. Never forward without subsequent parse.
pub async fn read_raw_frame<R: AsyncBufRead + Unpin>(
    input: &mut R,
    cap: usize,
) -> io::Result<Option<Vec<u8>>> {
    let mut result = Vec::new();
    loop {
        let buf = input.fill_buf().await?;
        if buf.is_empty() {
            return if result.is_empty() {
                Ok(None)
            } else {
                Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "partial_frame_eof",
                ))
            };
        }
        let newline = buf.iter().position(|b| *b == b'\n');
        let n = newline.map_or(buf.len(), |i| i + 1);
        if n > cap.saturating_sub(result.len()) {
            return Err(invalid("frame_limit"));
        }
        result.extend_from_slice(&buf[..n]);
        input.consume(n);
        if newline.is_some() {
            return Ok(Some(result));
        }
        if result.len() == cap {
            return Err(invalid("frame_limit_without_newline"));
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncWriteExt, BufReader};
    #[tokio::test]
    async fn exact_bounds_utf8_crlf_and_fragmentation_preserve_bytes() {
        let raw = "{\"text\":\"中文\"}\r\n".as_bytes();
        for cap in [raw.len() - 1, raw.len(), raw.len() + 1] {
            let mut input = BufReader::new(raw);
            let value = read_frame(&mut input, cap).await;
            if cap < raw.len() {
                assert!(value.is_err());
            } else {
                assert_eq!(value.unwrap().unwrap(), raw);
            }
        }
        let (a, mut b) = tokio::io::duplex(1);
        let expected = raw.to_vec();
        let bytes = expected.clone();
        let writer = tokio::spawn(async move {
            for byte in bytes {
                b.write_all(&[byte]).await.unwrap();
                tokio::task::yield_now().await;
            }
        });
        assert_eq!(
            read_frame(&mut BufReader::new(a), 64)
                .await
                .unwrap()
                .unwrap(),
            expected
        );
        writer.await.unwrap();
    }
    #[tokio::test]
    async fn malformed_partial_non_utf8_pollution_and_unterminated_caps_fail() {
        for raw in [
            &b"{\"x\":1}"[..],
            &b"{bad}\n"[..],
            &b"{\"x\":\"\xff\"}\n"[..],
            &b"diagnostic\n"[..],
            &b"[]\n"[..],
            &b"\n"[..],
            &b"123456789012345678901"[..],
        ] {
            assert!(
                read_frame(&mut BufReader::new(raw), 20).await.is_err(),
                "{raw:?}"
            );
        }
        assert!(
            read_frame(&mut BufReader::new(&b""[..]), 32)
                .await
                .unwrap()
                .is_none()
        );
    }
    #[tokio::test]
    async fn configured_attach_inbound_outbound_caps_include_delimiters() {
        for cap in [
            crate::frame::ATTACH_CAP,
            crate::frame::INBOUND,
            crate::frame::OUTBOUND,
        ] {
            let mut raw = b"{\"pad\":\"".to_vec();
            raw.resize(cap - 3, b'x');
            raw.extend_from_slice(b"\"}\n");
            assert_eq!(raw.len(), cap);
            assert!(
                read_frame(&mut BufReader::new(raw.as_slice()), cap - 1)
                    .await
                    .is_err()
            );
            for allowed in [cap, cap + 1] {
                assert_eq!(
                    read_frame(&mut BufReader::new(raw.as_slice()), allowed)
                        .await
                        .unwrap()
                        .unwrap(),
                    raw
                );
            }
            raw.pop();
            raw.push(b' ');
            assert!(
                read_frame(&mut BufReader::new(raw.as_slice()), cap)
                    .await
                    .is_err()
            );
        }
    }
}
