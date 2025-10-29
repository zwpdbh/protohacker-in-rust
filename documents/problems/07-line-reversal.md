## 7: Line Reversal

Problem 7: **Line Reversal** is not just about reversing strings—it’s a **deep test of your ability to implement a reliable transport protocol on top of an unreliable one**. Here’s what it’s *really* testing from a **networking and systems programming perspective**:

---

### 🔧 Core Networking Skills Being Tested

#### 1. **Reliable Transport over UDP**
- You must **reimplement TCP-like semantics** (in-order, reliable byte streams) using **UDP**, which provides **no guarantees**.
- This tests your understanding of:
  - **Sequence numbers** (`POS`)
  - **Acknowledgments** (`/ack/`)
  - **Retransmission on timeout**
  - **Duplicate detection**

> 💡 This is essentially **building a minimal TCP in user space**.

#### 2. **Stateful Session Management**
- Each session is identified by a token and tied to a `(IP, port)` tuple.
- You must maintain per-session state:
  - Received byte count (`LENGTH`)
  - Buffer of unprocessed data
  - Last ACK sent
  - Expiry timers

#### 3. **Sliding Window / Cumulative ACK Logic**
- You only ACK the **total number of contiguous bytes received** from the start.
- If you get data at position `10` but are missing `0–9`, you **re-ACK the last contiguous offset** (e.g., `0`) to trigger retransmission.
- This is **cumulative acknowledgment**, just like TCP.

#### 4. **Message Framing & Parsing over Datagram Protocol**
- LRCP uses **text-based, slash-delimited messages** (e.g., `/data/123/0/hello/`).
- You must:
  - Parse fields safely
  - Validate structure, length, and numeric bounds
  - **Silently drop invalid packets** (no error responses)

#### 5. **Application-Layer Stream Abstraction**
- Despite UDP’s packet nature, your app must present a **continuous byte stream**.
- You buffer partial lines until `\n` arrives, then **reverse complete lines** and send them back **in-order**.
- This decouples **transport reliability** from **application logic**.

#### 6. **Escape Sequence Handling**
- Forward slashes (`/`) and backslashes (`\`) are escaped as `\/` and `\\`.
- You must **unescape on receive**, **escape on send**—but **sequence numbers are based on unescaped byte count**.
- Tests attention to **layer separation**: transport counts *application bytes*, not wire bytes.

#### 7. **Timeout-Driven Retransmission (Optional but Implied)**
- While the server doesn’t *initiate* retransmission (clients do), you must **handle duplicate packets gracefully**.
- Understanding **retransmission timeout (RTO)** and **session expiry** shows you grasp real-world protocol robustness.

#### 8. **Concurrency & Resource Management**
- Support **20+ concurrent sessions**.
- Clean up expired sessions to avoid memory leaks.
- Map sessions by token + peer address securely.

---

### 🎯 Why This Matters
This problem tests whether you understand **how reliable protocols actually work under the hood**—beyond just using `TcpStream`. It’s a proxy for:
- Writing custom protocols (e.g., for games, IoT, or high-performance systems)
- Debugging network reliability issues
- Designing state machines for network protocols

> In short: **You’re not just writing a server—you’re writing a transport layer.**

This is one of the most **educational** challenges in Protohackers because it reveals the hidden complexity behind “simple” stream sockets. Mastering this means you truly understand **what TCP does for you**—and how to rebuild it when you can’t use it.

---

## The UDP to LRCP Abstraction 

1. Start with UdpSocket bind and receive

During LrcpListener's `bind`: 

- From UdpSocket, after `recv_from` got binary message and `SocketAddr`, which is `UdpPacketPair`.
- Parse binary message according to application binary message format, got `LrcpPacket`
- Now, produce `(LrcpPacket, SocketAddr)` which is `LrcpPacketPair`
  
So, `UdpPacketPair` -> `LrcpPacketPair` -> `LrcpStreamPair`

In particular, during `LrcpPacketPair` -> `LrcpStreamPair` is happened during `route_packet`.

2. `LrcpPacketPair` -> `LrcpStreamPair`

During `route_packet`: 

- Based on `lrcp_packet_pair.lrcp_packet`, create `LrcpStreamPair` if it is `LrcpPacket::Connect { session_id }`
- `LrcpStream` instance is created with `session_cmd_tx` and `bytes_rx`.
- `LrcpStreamPair` is created with `LrcpStreamPair::new(lrcp_stream, lrcp_packet_pair.addr)`.
- In addition, a `Session` is spawned, we will check it later. 

## Understanding LRCP Stream 

```rust 
pub struct LrcpStream {
    pub session_cmd_tx: mpsc::UnboundedSender<SessionCommand>,
    pub bytes_rx: mpsc::UnboundedReceiver<Bytes>,
    pub read_buf: Bytes,
}
```

- Used to send write requests (via SessionCommand::Write) to some session/task that actually writes to a real socket.
- `bytes_rx`
  - Used to receive incoming data (as Bytes) from that same session/task, which reads from the socket.
  - The `bytes_tx` is passed in `Session::spawn`.


## Session 

- When a session is created? 
 
  During `route_packet`:  a `Session` is spawned with 
  - `session_id`
  - `session_cmd_rx`
  - `session_event_rx`
  - `udp_packet_paire_tx`
  - `bytes_tx`, the other side is passed in `LrcpStream`.
  When the `lrcp_packet_pair.lrcp_packet` is a  `LrcpPacket::Connect { session_id }`.

- Its purpose: manages the state of a single logical connection.

## Data Flow 

### Write Path: from App -> to Network 

```rust
LrcpStream.poll_write()
  → sends SessionCommand::Write { data, reply_tx }
  → stores reply_rx for later polling
  → returns Poll::Pending

Later, when polled again:
  → polls reply_rx
  → Session processes command (queues data, sends UDP packet)
  → Session calls reply_tx.send(Ok(len))
  → poll_write receives result and returns Poll::Ready(Ok(len))
```

### Read Path: from Network -> to App 

```rust 
UDP socket receives datagram
  → UDP I/O task parses it into LrcpPacket
  → Session router forwards it as SessionEvent::Data { pos, escaped_data }
  → Session actor validates sequence number (pos == recv_pos)
  → Session unescapes data and sends it via bytes_tx.send(Bytes::from(unescaped))
  → LrcpStream's read_rx receives the Bytes
  → poll_read() pulls from read_rx (or read_buf) and fills ReadBuf
  → returns Poll::Ready(Ok(())) with data available to app
```

## The state of session 

You're implementing a reliable, ordered, bidirectional byte stream over UDP 
using your own protocol (LRCP), which is essentially a simplified version of TCP. 
The state management in session is crucial. 

- why need 3 postion counters 
  Because sending and receiving are independent, and we must track what we have sent 
  vs what has been acknowledged to handle retransmission.

- When this position counter get updated? 
  - `in_pos`, when `/data/` packet arrives in order. 
  - `out_pos`, whenever send new application data in `SessionCommand::Write { data, reply }`
  - `acked_out_pos`, only when a valid, non-duplicate `/ack/` arrives.


- why `pending_out_data` is necessary 
  - SessionCommand::Write → stored in pending_out_data → sent as /data/ packets.
  - It holds unacknowledged outgoing bytes that may need to be retransmitted.
  - Hence:
    - When you call write(), you append to pending_out_data.
    - You send it as a /data/ message with position = out_pos.
    - When an /ack/ arrives, you trim the acknowledged prefix from pending_out_data.
    - On timeout (Retransmit event), you resend the entire pending_out_data (or properly chunked parts).

- `bytes_tx`, is used to send received data upto the application layer.
  - Incoming data (client → server): parsed → unescaped → sent via bytes_tx → consumed by your line-reversal app.


### 🔄 Example Flow (Server Sending "olleh\n")

1. App calls `stream.write(b"olleh\n")`.
2. Session receives `SessionCommand::Write`, appends to `pending_out_data = b"olleh\n"`.
3. `out_pos = 0`, so it sends: `/data/123/0/olleh\n/`
4. `out_pos` becomes `6`.
5. If ACK is received (`/ack/123/6/`):
   - `acked_out_pos = 6`
   - `pending_out_data` is cleared (since all sent data is acknowledged).
6. If no ACK within 3s → `Retransmit` event → resend `/data/123/0/olleh\n/`.


### 🔄 Revised Example Flow: Partial ACK

**Goal**: Server sends `"olleh\n"` (6 bytes), but client only acknowledges 3 bytes initially.

#### Step 1: App writes data
```rust
stream.write(b"olleh\n").await?; // 6 bytes
```

#### Step 2: Session buffers it
- `pending_out_data = b"olleh\n"` (6 bytes)
- `out_pos = 0`
- `acked_out_pos = 0`

#### Step 3: Server sends first chunk (maybe due to size or design)
Suppose your implementation **splits large writes** (or you’re simulating loss), so it sends only the first 3 bytes:

- Sends: `/data/123/0/oll/`
- Updates:
  - `out_pos = 3` (we’ve sent 3 bytes so far)
  - `pending_out_data` still holds full `b"olleh\n"` (or just the unsent suffix—see note below)

> 💡 **Implementation note**: Ideally, `pending_out_data` should only contain **unsent + unacked** data. But in your current code, you send the *entire* buffer every time, which works for small messages but isn’t efficient. For correctness in this example, we’ll assume you’re tracking **what’s been sent** vs **what remains**.

To make partial ACKs meaningful, let’s assume you **send incrementally**:

- First, send `b"oll"` → `out_pos = 3`
- Later, send `b"eh\n"` → `out_pos = 6`

But for simplicity, let’s say you sent all 6 bytes in one packet, yet the client **only processed 3 bytes** (perhaps due to an internal buffer limit—though LRCP spec says ACK reflects total received, so this is a bit artificial).  

However, **per the LRCP spec**, the client **must ACK the total number of bytes received**, so a partial ACK like `3` implies that only the first 3 bytes of the stream were received—meaning the `/data/` message either:
- Was truncated (invalid), or
- You actually sent **two `/data/` messages**: one at pos=0 (3 bytes), another at pos=3 (3 bytes), and the second was lost.

So let’s use the **correct LRCP-compliant scenario**:

---

### ✅ Correct LRCP Example: Two Data Packets, One Lost

#### Step 1: App writes `"olleh\n"` (6 bytes)

#### Step 2: Server splits into two messages (e.g., due to internal buffering or MTU)
- Sends `/data/123/0/oll/` → 3 bytes
- Sends `/data/123/3/eh\n/` → 3 bytes
- Now:
  - `out_pos = 6`
  - `pending_out_data = b"olleh\n"` (or better: you track sent ranges)

#### Step 3: Client receives **only the first packet**
- Client has bytes `[0..3)` → `"oll"`
- Sends ACK: `/ack/123/3/`

#### Step 4: Server receives `/ack/123/3/`
- `length = 3`
- Since `3 > acked_out_pos (0)`, update:
  - `acked_out_pos = 3`
- Now, **trim acknowledged prefix** from `pending_out_data`:
  - Remove first 3 bytes → `pending_out_data = b"eh\n"`

> 🔍 This is why you **must** keep `pending_out_data`: to know what still needs ACKing.

#### Step 5: Retransmit timer fires (3s later)
- `pending_out_data = b"eh\n"` is not empty
- Resend it at position `out_pos - pending_out_data.len() = 6 - 3 = 3`
- Sends: `/data/123/3/eh\n/`

#### Step 6: Client receives it, now has 6 bytes
- Sends `/ack/123/6/`

#### Step 7: Server receives final ACK
- `acked_out_pos = 6`
- `pending_out_data` is cleared
- Transmission complete ✅

---

### 🧠 Key Takeaway

A partial ACK (`length = 3` instead of `6`) tells the server:
> “I’ve safely received bytes 0 through 2. Anything from byte 3 onward may be missing—please retransmit.”

And `pending_out_data` (or a more advanced structure like a send buffer with byte ranges) is **essential** to know **what to retransmit**.

Without it, you’d either:
- Retransmit everything (inefficient), or
- Lose data (unreliable).

Your current design uses a simple `Vec<u8>` for `pending_out_data`, which works if you **always send from the beginning of the unacked region**—but for robustness, you’ll eventually want to track **which byte ranges have been sent** (like TCP’s send buffer). For protohacker, the simple model is likely sufficient.

--- 


### 🧠 Summary

| Concept            | Purpose                                                                          |
| ------------------ | -------------------------------------------------------------------------------- |
| `in_pos`           | Tracks how much **incoming** data has been received (for ACKs & detecting gaps). |
| `out_pos`          | Total bytes **sent** (used as the `POS` in `/data/` messages).                   |
| `acked_out_pos`    | Bytes **confirmed received** by peer (used to trim `pending_out_data`).          |
| `pending_out_data` | Buffer of **unacknowledged outgoing data** needed for **retransmission**.        |
| `bytes_tx`         | **Incoming** data channel (network → app), **not** for outgoing traffic.         |

You **cannot** avoid `pending_out_data` if you want LRCP to be reliable. It’s the core of your "fake TCP over UDP" implementation.


## Troubleshooting 

Same test produce inconsistent result:

```sh 
cargo test --package protohacker-in-rust --test lrcp_e2e -- line_reversal_tests::test_line_reversal_session --exact --nocapture 
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.03s
     Running tests/lrcp_e2e.rs (target/debug/deps/lrcp_e2e-ceea6c6c98a3ab9b)

running 1 test
2025-10-29T09:25:37.243955Z DEBUG protohacker_in_rust::protohackers::problem7::lrcp::listener: <<- received lrcp_packet: Connect { session_id: 12345 }
2025-10-29T09:25:37.244037Z DEBUG protohacker_in_rust::protohackers::problem7::lrcp::listener: ->> send udp_packet: UdpPacketPair -- target: 127.0.0.1:46738, payload: /ack/12345/0/
2025-10-29T09:25:37.244122Z DEBUG handle_session: protohacker_in_rust::protohackers::problem7::server: Waiting for next line...
2025-10-29T09:25:37.244314Z DEBUG handle_session: protohacker_in_rust::protohackers::problem7::lrcp::listener: <<- received lrcp_packet: Data { session_id: 12345, pos: 0, escaped_data: "hello\n" }
2025-10-29T09:25:37.244400Z DEBUG handle_session: protohacker_in_rust::protohackers::problem7::server: read_line returned 6 bytes
2025-10-29T09:25:37.244423Z DEBUG handle_session: protohacker_in_rust::protohackers::problem7::server: reversed: olleh
2025-10-29T09:25:37.244443Z DEBUG handle_session: protohacker_in_rust::protohackers::problem7::lrcp::stream: Starting new write of 6 bytes
2025-10-29T09:25:37.244466Z DEBUG handle_session: protohacker_in_rust::protohackers::problem7::lrcp::stream: Write command sent to session, waiting for acknowledgment
2025-10-29T09:25:37.244490Z DEBUG handle_session: protohacker_in_rust::protohackers::problem7::lrcp::listener: ->> send udp_packet: UdpPacketPair -- target: 127.0.0.1:46738, payload: /ack/12345/6/
2025-10-29T09:25:37.244537Z DEBUG handle_session: protohacker_in_rust::protohackers::problem7::lrcp::listener: ->> send udp_packet: UdpPacketPair -- target: 127.0.0.1:46738, payload: /data/12345/0/olleh
/
2025-10-29T09:25:37.244845Z DEBUG handle_session: protohacker_in_rust::protohackers::problem7::lrcp::listener: <<- received lrcp_packet: Ack { session_id: 12345, length: 6 }
2025-10-29T09:25:37.244887Z DEBUG handle_session: protohacker_in_rust::protohackers::problem7::lrcp::listener: <<- received lrcp_packet: Data { session_id: 12345, pos: 6, escaped_data: "Hello, world!\n" }
2025-10-29T09:25:37.244933Z DEBUG handle_session: protohacker_in_rust::protohackers::problem7::lrcp::listener: ->> send udp_packet: UdpPacketPair -- target: 127.0.0.1:46738, payload: /ack/12345/20/
Error: Other("Timeout waiting for second server data")
test line_reversal_tests::test_line_reversal_session ... FAILED
```


```sh 
cargo test --package protohacker-in-rust --test lrcp_e2e -- line_reversal_tests::test_line_reversal_session --exact --nocapture 
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.03s
     Running tests/lrcp_e2e.rs (target/debug/deps/lrcp_e2e-ceea6c6c98a3ab9b)

running 1 test
2025-10-29T09:25:33.645923Z DEBUG protohacker_in_rust::protohackers::problem7::lrcp::listener: <<- received lrcp_packet: Connect { session_id: 12345 }
2025-10-29T09:25:33.646008Z DEBUG protohacker_in_rust::protohackers::problem7::lrcp::listener: ->> send udp_packet: UdpPacketPair -- target: 127.0.0.1:44639, payload: /ack/12345/0/
2025-10-29T09:25:33.646091Z DEBUG handle_session: protohacker_in_rust::protohackers::problem7::server: Waiting for next line...
2025-10-29T09:25:33.646542Z DEBUG handle_session: protohacker_in_rust::protohackers::problem7::lrcp::listener: <<- received lrcp_packet: Data { session_id: 12345, pos: 0, escaped_data: "hello\n" }
2025-10-29T09:25:33.646630Z DEBUG handle_session: protohacker_in_rust::protohackers::problem7::server: read_line returned 6 bytes
2025-10-29T09:25:33.646658Z DEBUG handle_session: protohacker_in_rust::protohackers::problem7::server: reversed: olleh
2025-10-29T09:25:33.646669Z DEBUG handle_session: protohacker_in_rust::protohackers::problem7::lrcp::stream: Starting new write of 6 bytes
2025-10-29T09:25:33.646677Z DEBUG handle_session: protohacker_in_rust::protohackers::problem7::lrcp::stream: Write command sent to session, waiting for acknowledgment
2025-10-29T09:25:33.646686Z DEBUG handle_session: protohacker_in_rust::protohackers::problem7::lrcp::listener: ->> send udp_packet: UdpPacketPair -- target: 127.0.0.1:44639, payload: /ack/12345/6/
2025-10-29T09:25:33.646712Z DEBUG handle_session: protohacker_in_rust::protohackers::problem7::lrcp::listener: ->> send udp_packet: UdpPacketPair -- target: 127.0.0.1:44639, payload: /data/12345/0/olleh
/
2025-10-29T09:25:33.646952Z DEBUG handle_session: protohacker_in_rust::protohackers::problem7::lrcp::listener: ->> send udp_packet: UdpPacketPair -- target: 127.0.0.1:44639, payload: /data/12345/0/olleh
/
2025-10-29T09:25:33.647224Z DEBUG handle_session: protohacker_in_rust::protohackers::problem7::lrcp::listener: <<- received lrcp_packet: Ack { session_id: 12345, length: 6 }
2025-10-29T09:25:33.647260Z DEBUG handle_session: protohacker_in_rust::protohackers::problem7::lrcp::listener: <<- received lrcp_packet: Data { session_id: 12345, pos: 6, escaped_data: "Hello, world!\n" }
2025-10-29T09:25:33.647303Z DEBUG handle_session: protohacker_in_rust::protohackers::problem7::lrcp::listener: ->> send udp_packet: UdpPacketPair -- target: 127.0.0.1:44639, payload: /ack/12345/20/

thread 'line_reversal_tests::test_line_reversal_session' panicked at tests/lrcp_e2e.rs:83:9:
assertion `left == right` failed
  left: "/data/12345/0/olleh\n/"
 right: "/ack/12345/20/"
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace
test line_reversal_tests::test_line_reversal_session ... FAILED
```