use super::session::SessionCommand;
use bytes::Bytes;
use std::net::SocketAddr;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::sync::mpsc;
use tracing::Level;
use tracing::span;

pub struct LrcpStreamPair {
    pub stream: LrcpStream,
    pub addr: SocketAddr,
}
impl LrcpStreamPair {
    pub fn new(stream: LrcpStream, addr: SocketAddr) -> Self {
        LrcpStreamPair { stream, addr }
    }
}

// Application-facing, for integration with higher-level code
pub struct LrcpStream {
    pub session_cmd_tx: mpsc::UnboundedSender<SessionCommand>,
    // For reads, you'd add: read_rx: mpsc::UnboundedReceiver<Vec<u8>>
    // But for line reversal, you might just buffer in session and expose lines
    pub read_rx: mpsc::UnboundedReceiver<Bytes>,
    // Buffer for partial reads (important!)
    pub read_buf: Bytes,
    // // ✅ New: store the pending write reply future
    // pending_write: Option<oneshot::Receiver<std::io::Result<usize>>>,
}

impl LrcpStream {
    pub(crate) fn new(
        cmd_tx: mpsc::UnboundedSender<SessionCommand>,
        read_rx: mpsc::UnboundedReceiver<Bytes>,
    ) -> Self {
        Self {
            session_cmd_tx: cmd_tx,
            read_rx,
            read_buf: Bytes::new(),
            // pending_write: None,
        }
    }
}

// Make sure LrcpStream is Unpin (it is, by default, since no !Unpin fields)
impl Unpin for LrcpStream {}
impl AsyncWrite for LrcpStream {
    fn poll_write(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        let cmd = SessionCommand::Write { data: buf.to_vec() };
        if self.session_cmd_tx.send(cmd).is_err() {
            return Poll::Ready(Err(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "closed",
            )));
        }
        Poll::Ready(Ok(buf.len()))
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        // For now, flush is a no-op since writes are immediate
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        // Optional: send shutdown command
        Poll::Ready(Ok(()))
    }
}

impl AsyncRead for LrcpStream {
    /// handles the raw byte streaming from the session to the application
    /// `BufReader::read_line()`` calls `poll_read()`
    /// BufReader has an internal buffer (typically 8KB)
    /// It calls poll_read() with a large buf (enough to hold the entire line)
    /// The poll_read method has a very specific contract:
    /// "Copy available data into the provided buffer and return immediately — don't wait for more data."
    /// You only fill the caller's buffer (buf)
    /// You return immediately with whatever data you have
    /// You do NOT wait for a complete line, message, or any specific amount
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        let span = span!(Level::INFO, "poll_read");
        let _enter = span.enter();

        let this = self.get_mut();

        // Checked buffered data
        // If there's leftover data from a previous read (because the buffer was too small), use it first
        // This handles the case where one network packet contains multiple lines
        if !this.read_buf.is_empty() {
            let len = std::cmp::min(buf.remaining(), this.read_buf.len());
            buf.put_slice(&this.read_buf[..len]);
            this.read_buf = this.read_buf.slice(len..);
            return Poll::Ready(Ok(()));
        }

        // Receive new data
        match this.read_rx.poll_recv(cx) {
            Poll::Ready(Some(bytes)) => {
                let len = std::cmp::min(buf.remaining(), bytes.len());
                buf.put_slice(&bytes[..len]);
                if len < bytes.len() {
                    // Buffer the rest for next read
                    this.read_buf = bytes.slice(len..);
                }
                Poll::Ready(Ok(()))
            }
            Poll::Ready(None) => {
                // Channel closed → EOF
                Poll::Ready(Ok(()))
            }
            Poll::Pending => Poll::Pending,
        }
    }
}
