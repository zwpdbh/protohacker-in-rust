## 4: Unusual Database Program


1. **UDP Socket Programming**  
   – Used `tokio::net::UdpSocket` with `recv_from()`/`send_to()` for **message-oriented, connectionless I/O**.

2. **Stateful Shared Storage with Concurrency Safety**  
   – Wrapped a `HashMap` in `Arc<Mutex<...>>` to allow safe mutation across async tasks (though here it’s single-threaded, the pattern is correct).

3. **Protocol Parsing from Raw Bytes**  
   – Handled **UTF-8 validation** (`str::from_utf8`) and **delimited parsing** (`find('=')`) on raw datagrams.

4. **Immutable Special-Key Handling**  
   – Enforced read-only semantics for `"version"` by **ignoring insert requests** during parse.

5. **Best-Effort UDP Semantics**  
   – Silently dropped malformed or invalid packets (e.g., non-UTF-8), aligning with UDP’s unreliable nature.

6. **Request/Response Logic per Datagram**  
   – Each UDP packet is independent: **no session state**, no streaming—pure request → (optional) response.

7. **Memory-Efficient Buffer Reuse**  
   – Used a single large buffer (`vec![0u8; 65536]`) reused across receives—efficient and idiomatic.

8. **Clear Separation of Concerns**  
   – Split logic into `protocol.rs` (parsing/formatting) and `server.rs` (I/O + storage)—clean architecture.

9. **Comprehensive Unit Testing**  
   – Tested edge cases: empty keys/values, `=` in values, version protection, UTF-8 errors, updates.

10. **Graceful Degradation on Missing Keys**  
    – Chose to respond with `"key="` for missing keys (allowed by spec), ensuring predictable client behavior.
