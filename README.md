# Protohackers 

## 1: Prime Time 

1. **Async TCP Server with Tokio**  
   – Accepted connections and spawned async tasks per client.

2. **Line-Oriented Message Framing**  
   – Used `BufReader::lines()` to correctly parse `\n`-delimited messages from a byte stream.

3. **Stream Splitting for Buffered Reads**  
   – Split `TcpStream` into separate read/write halves to enable `BufReader` without blocking writes.

4. **Protocol Parsing with Serde + JSON**  
   – Deserialized incoming JSON lines into strongly typed structs; enforced schema via `Deserialize`.

5. **Stateless Request/Response Handling**  
   – Processed each line independently in a loop, maintaining no per-client state beyond the connection.

6. **Protocol-Compliant Error Handling**  
   – Detected malformed requests (parse errors) and **terminated the connection immediately** after one error response.

7. **Flushed Writes for Correct Timing**  
   – Called `.flush().await` to ensure responses are sent promptly (important for request/response protocols).

8. **Testable I/O Abstraction**  
   – Wrote core logic against `AsyncRead`/`AsyncWrite` traits, enabling unit tests with in-memory byte buffers.

9. **Graceful Connection Teardown**  
   – Let the async task end naturally on EOF or error, relying on RAII to close the socket.


## 2: Means to an End 

1. **Fixed-Length Binary Protocol Handling**  
   – Used `read_exact(&mut [u8; 9])` to read **exactly 9 bytes per message**, enforcing strict wire format.

2. **Manual Binary Serialization/Deserialization**  
   – Parsed raw bytes using `i32::from_be_bytes()` and constructed responses with `to_be_bytes()` (big-endian network order).

3. **Stateful Per-Connection Server**  
   – Maintained an in-memory `BTreeMap` database **per client session** (not shared across connections).

4. **Efficient Range Queries with `BTreeMap::range()`**  
   – Leveraged ordered map for O(log n) time-range lookups and computed mean on-the-fly.

5. **Stream Splitting for Read/Write Separation**  
   – Split `TcpStream` to allow independent reading (with `read_exact`) and writing (binary responses).

6. **Graceful EOF Handling**  
   – Detected client disconnect via `UnexpectedEof` and exited cleanly without error.

7. **Custom Error Types for Protocol Violations**  
   – Defined `Error::InvalidProtocol` to distinguish malformed messages from I/O errors.

8. **Async I/O with Tokio Primitives**  
   – Built a concurrent server using `TcpListener`, `tokio::spawn`, and async I/O traits.

9. **Integration Testing with Raw Byte Buffers**  
   – Simulated real client sessions by feeding concatenated binary messages into `handle_client_internal`.

10. **Zero-Copy Parsing (Stack-Allocation)**  
    – Parsed messages from a fixed-size stack buffer (`[u8; 9]`), avoiding heap allocation per message.

## 3. Budget Chat 

1. **Async Message-Passing Architecture**  
   – Used `tokio::sync::mpsc` channels to decouple client I/O from shared room logic (actor-like design).

2. **Centralized State Management**  
   – Implemented a dedicated `run_room` task as the single source of truth for chat state (users, presence, broadcasts).

3. **Protocol Abstraction with Codecs**  
   – Leveraged `tokio_util::codec::Framed` + custom `ChatCodec` to cleanly separate **framing** (`\n`-delimited lines) from business logic.

4. **Structured Message Types**  
   – Defined clear enums (`OutgoingMessage`, `RoomMessage`) to model all possible server→client and internal events.

5. **Client Lifecycle Handling**  
   – Managed two-phase state: unjoined (awaiting username) → joined (in chat), with proper validation and cleanup.

6. **Selective Broadcast Logic**  
   – Correctly excluded senders from chat echoes and unjoined clients from presence notifications.

7. **Graceful Disconnection & Cleanup**  
   – Detected stream closure (`None` from `Stream`) and triggered `room.leave()` to notify others.

8. **Concurrent Client Support**  
   – Spawned one task per client while sharing a single `Room` instance via `Clone` (backed by channel sender).

9. **Comprehensive Integration Testing**  
   – Simulated full client sessions using mocked streams/sinks and verified message ordering and content.

10. **Display-Driven Serialization**  
    – Used `derive_more::Display` to automatically format outgoing messages (e.g., `"[alice] hello"`), reducing manual string building.


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


## 6: Speed Daemon

1. **Complex Binary Protocol Implementation**  
   – Manually encoded/decoded **network-byte-order** messages with mixed types (`u8`, `u16`, `u32`, length-prefixed strings) without external serialization (e.g., no Bincode for wire format).

2. **Stateful, Multi-Role Server Architecture**  
   – Supported two distinct client roles (**cameras** and **dispatchers**) with strict state validation (e.g., only cameras send `Plate`).

3. **Centralized Global State with Message Passing**  
   – Used a dedicated `run_state` task with `mpsc::UnboundedChannel` as the single source of truth for:
     - Client registry,
     - Road-to-dispatcher mapping,
     - Plate observation history,
     - Ticket deduplication.

4. **Efficient Time-Series Storage & Querying**  
   – Stored per-plate, per-road observations in `BTreeMap<Timestamp, Mile>` for **ordered, range-aware speed calculations**.

5. **Domain-Logic-Driven Ticket Generation**  
   – Implemented **average speed calculation**, **daily ticket deduplication**, and **multi-day span handling** per spec.

6. **Pending Ticket Buffering & Delivery**  
   – Queued tickets when no dispatcher is available and **flushed on dispatcher registration**—ensuring no loss.

7. **Heartbeat Subsystem with Cancellation**  
   – Used `tokio::time::interval` + `oneshot` channel to manage **per-client heartbeats** that auto-stop on disconnect.

8. **Strict Protocol Validation & Error Enforcement**  
   – Enforced one-time role assignment, disallowed duplicate `WantHeartbeat`, and sent `Error` + disconnect on violations.

9. **Zero-Copy Message Routing**  
   – Separated **network I/O** (`handle_client`) from **business logic** (`run_state`) via internal message enums—enabling testability and clarity.

10. **High-Concurrency Design (150+ Clients)**  
    – Leveraged async tasks + shared state via channels—**no locks on hot paths**, scalable to required client count.



## References 

- [PROTOHACKERS](https://protohackers.com/)