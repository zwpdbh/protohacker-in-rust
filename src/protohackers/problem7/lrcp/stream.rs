use super::protocol::LrcpPacket;
use super::session::SessionCommand;
use bytes::Bytes;
use futures::FutureExt;
use std::net::SocketAddr;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::sync::mpsc;
use tokio::sync::oneshot;

pub struct LrcpStreamPair {
    pub stream: LrcpStream,
    pub addr: SocketAddr,
}
impl LrcpStreamPair {
    pub fn new(stream: LrcpStream, addr: SocketAddr) -> Self {
        LrcpStreamPair { stream, addr }
    }
}

#[derive(Debug)]
pub struct LrcpPacketPair {
    pub lrcp_packet: LrcpPacket,
    pub addr: SocketAddr,
}
impl LrcpPacketPair {
    #[allow(unused)]
    pub fn new(packet: LrcpPacket, addr: SocketAddr) -> Self {
        LrcpPacketPair {
            lrcp_packet: packet,
            addr,
        }
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

    // ✅ New: store the pending write reply future
    pending_write: Option<oneshot::Receiver<std::io::Result<usize>>>,
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
            pending_write: None,
        }
    }
}

// Make sure LrcpStream is Unpin (it is, by default, since no !Unpin fields)
impl Unpin for LrcpStream {}
impl AsyncWrite for LrcpStream {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        let this = self.get_mut();

        // 🔁 If we're still waiting for a previous write to complete,
        // we must NOT start a new one (AsyncWrite assumes sequential writes).
        if let Some(mut receiver) = this.pending_write.take() {
            match receiver.poll_unpin(cx) {
                Poll::Ready(Ok(res)) => return Poll::Ready(res),
                Poll::Ready(Err(_)) => {
                    // oneshot canceled → session dropped
                    return Poll::Ready(Err(std::io::Error::new(
                        std::io::ErrorKind::BrokenPipe,
                        "session closed",
                    )));
                }
                Poll::Pending => {
                    // Put it back and wait
                    this.pending_write = Some(receiver);
                    return Poll::Pending;
                }
            }
        }

        // 📤 No pending write → send new one
        let (reply_tx, reply_rx) = oneshot::channel();
        let session_cmd = SessionCommand::Write {
            data: buf.to_vec(),
            reply: reply_tx,
        };

        if this.session_cmd_tx.send(session_cmd).is_err() {
            return Poll::Ready(Err(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "session closed",
            )));
        }

        // Store the receiver to poll next time
        this.pending_write = Some(reply_rx);
        Poll::Pending // We'll get woken when reply_rx is ready
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

// AsyncRead is harder — you'd need the session to push data into a channel
// that LrcpStream reads from. Left as exercise.
impl AsyncRead for LrcpStream {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        let this = self.get_mut();

        // If we have buffered data, use it first
        if !this.read_buf.is_empty() {
            let len = std::cmp::min(buf.remaining(), this.read_buf.len());
            buf.put_slice(&this.read_buf[..len]);
            this.read_buf = this.read_buf.slice(len..);
            return Poll::Ready(Ok(()));
        }

        // Otherwise, try to receive a new chunk
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
