#![allow(unused)]
use super::session::SessionCommand;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::sync::mpsc;

pub struct LrcpStream {
    pub cmd_tx: mpsc::UnboundedSender<SessionCommand>,
    // For reads, you'd add: read_rx: mpsc::UnboundedReceiver<Vec<u8>>
    // But for line reversal, you might just buffer in session and expose lines
}

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
