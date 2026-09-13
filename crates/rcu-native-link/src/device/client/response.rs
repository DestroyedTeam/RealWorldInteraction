//! Keep framing unchanged; a disconnect after any response byte is not retryable.
use super::*;
use std::{
    pin::Pin,
    task::{Context, Poll},
};
use tokio::io::{AsyncBufRead, AsyncRead, ReadBuf};

struct Progress<'a, R> {
    input: &'a mut R,
    consumed: usize,
}
impl<R: AsyncRead + Unpin> AsyncRead for Progress<'_, R> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        let before = buf.filled().len();
        let result = Pin::new(&mut *this.input).poll_read(cx, buf);
        this.consumed = this.consumed.saturating_add(buf.filled().len() - before);
        result
    }
}

impl<R: AsyncBufRead + Unpin> AsyncBufRead for Progress<'_, R> {
    fn poll_fill_buf(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<&[u8]>> {
        Pin::new(&mut *self.get_mut().input).poll_fill_buf(cx)
    }
    fn consume(self: Pin<&mut Self>, amount: usize) {
        let this = self.get_mut();
        this.consumed = this.consumed.saturating_add(amount);
        Pin::new(&mut *this.input).consume(amount);
    }
}
pub(super) async fn read<R: AsyncBufRead + Unpin>(
    input: &mut R,
) -> Result<Option<Vec<u8>>, Failure> {
    let mut progress = Progress { input, consumed: 0 };
    crate::wire::read_frame(&mut progress, RESPONSE_CAP)
        .await
        .map_err(|error| {
            if progress.consumed == 0 {
                Failure::transport(error)
            } else {
                Failure::other(error)
            }
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Cut {
        prefix: &'static [u8],
        kind: io::ErrorKind,
    }
    impl AsyncRead for Cut {
        fn poll_read(
            self: Pin<&mut Self>,
            _: &mut Context<'_>,
            _: &mut ReadBuf<'_>,
        ) -> Poll<io::Result<()>> {
            panic!("framer must use bounded buffered reads")
        }
    }
    impl AsyncBufRead for Cut {
        fn poll_fill_buf(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<&[u8]>> {
            let this = self.get_mut();
            if this.prefix.is_empty() {
                Poll::Ready(Err(io::Error::from(this.kind)))
            } else {
                Poll::Ready(Ok(this.prefix))
            }
        }
        fn consume(self: Pin<&mut Self>, amount: usize) {
            let this = self.get_mut();
            this.prefix = &this.prefix[amount..];
        }
    }

    #[tokio::test]
    async fn only_explicit_zero_byte_disconnects_can_be_retry_candidates() {
        for kind in [
            io::ErrorKind::BrokenPipe,
            io::ErrorKind::ConnectionReset,
            io::ErrorKind::ConnectionAborted,
            io::ErrorKind::NotConnected,
            io::ErrorKind::UnexpectedEof,
            io::ErrorKind::TimedOut,
            io::ErrorKind::InvalidData,
            io::ErrorKind::PermissionDenied,
            io::ErrorKind::Other,
        ] {
            for prefix in [&b""[..], &b"{\"partial\":"[..]] {
                let mut input = Cut { prefix, kind };
                let Err(failure) = read(&mut input).await else {
                    panic!("cut returned a frame")
                };
                let eligible = matches!(
                    kind,
                    io::ErrorKind::BrokenPipe
                        | io::ErrorKind::ConnectionReset
                        | io::ErrorKind::ConnectionAborted
                        | io::ErrorKind::NotConnected
                );
                assert_eq!(
                    failure.disconnected,
                    prefix.is_empty() && eligible,
                    "{kind:?} {prefix:?}"
                );
                assert_eq!(failure.error.kind(), kind);
            }
        }
        assert!(!Failure::other(invalid("device_call_timeout_effect_unknown")).disconnected);
    }
}
