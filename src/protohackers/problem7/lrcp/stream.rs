#![allow(unused)]
use super::session::SessionCommand;
use bytes::Bytes;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::sync::mpsc;

pub struct LrcpStream {
    pub cmd_tx: mpsc::UnboundedSender<SessionCommand>,
    // For reads, you'd add: read_rx: mpsc::UnboundedReceiver<Vec<u8>>
    // But for line reversal, you might just buffer in session and expose lines
    pub read_rx: mpsc::UnboundedReceiver<Bytes>,
    // Buffer for partial reads (important!)
    pub read_buf: Bytes,
}

impl LrcpStream {
    pub(crate) fn new(
        cmd_tx: mpsc::UnboundedSender<SessionCommand>,
        read_rx: mpsc::UnboundedReceiver<Bytes>,
    ) -> Self {
        Self {
            cmd_tx,
            read_rx,
            read_buf: Bytes::new(),
        }
    }
}

// Make sure LrcpStream is Unpin (it is, by default, since no !Unpin fields)
impl Unpin for LrcpStream {}
impl AsyncWrite for LrcpStream {
    fn poll_write(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<Result<usize, std::io::Error>> {
        let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
        let cmd = SessionCommand::Write {
            data: buf.to_vec(),
            reply: reply_tx,
        };
        self.cmd_tx
            .send(cmd)
            .map_err(|_| std::io::Error::new(std::io::ErrorKind::BrokenPipe, "session closed"))?;

        // This is blocking! Not ideal for AsyncWrite.
        // Better: buffer writes locally and flush via task.
        // For Protohackers, you can get away with fire-and-forget writes.
        std::task::Poll::Ready(Ok(buf.len()))
    }

    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        std::task::Poll::Ready(Ok(()))
    }

    fn poll_shutdown(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        std::task::Poll::Ready(Ok(()))
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
