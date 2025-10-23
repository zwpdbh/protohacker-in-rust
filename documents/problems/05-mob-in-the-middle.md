## 5: Mob in the Middle

1. **TCP Proxy Architecture**  
   – Built a bidirectional proxy that relays messages between client and upstream server in real time.

2. **Concurrent Bidirectional Relaying**  
   – Used `tokio::select!` to **simultaneously forward traffic in both directions** without blocking.

3. **Stream Splitting with Framed Codecs**  
   – Leveraged `tokio_util::codec::Framed` + `LinesCodec` to handle **line-delimited Budget Chat messages** cleanly on both sides.

4. **Protocol-Aware Message Rewriting**  
   – Applied **regex-based pattern matching** to detect and replace Boguscoin addresses while preserving message structure.

5. **Correct Regex Boundary Handling**  
   – Used `(^| )` and `($| )` to enforce **word-boundary-like constraints** (as required by the spec) without `\b` (which doesn’t work reliably on alphanumeric-only tokens).

6. **Graceful Connection Teardown**  
   – Broke the relay loop on **any stream closure or error**, ensuring both sides are dropped (Rust’s RAII closes sockets).

7. **Error Isolation & Logging**  
   – Logged directional errors (`client_stream` vs `upstream_stream`) for debugging while maintaining proxy stability.

8. **Async I/O with Tokio Ecosystem**  
   – Integrated `TcpStream`, `Framed`, `Sink`, `Stream`, and `select!` idiomatically—showing fluency in async Rust patterns.

9. **Zero Shared State per Session**  
   – Each proxy session is **self-contained**: no global state, no channels—just two streams and a loop.

10. **Real-World Protocol Interception**  
    – Demonstrated understanding of **man-in-the-middle** design: transparent relay + targeted payload mutation.
