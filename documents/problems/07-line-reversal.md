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

## My Notes 

