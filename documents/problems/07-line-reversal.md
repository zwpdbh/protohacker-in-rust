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




## Elixir solution


### 🎯 Goal Recap
We want to write application code like this:
```elixir
# From Protohacker.LineReversal.Acceptor
 def run(ip, port) when is_tuple(ip) and is_integer(port) do
    case LRCP.listen(ip, port) do
      {:ok, %LRCP.ListenSocket{} = listen_socket} ->
        accept_loop(listen_socket)

      {:error, reason} ->
        raise "failed to start LRCP listen socket on port #{port}: #{inspect(reason)}"
    end
  end

  defp accept_loop(%LRCP.ListenSocket{} = listen_socket) do
    # We keep call :accept on GenServer listen_socket
    case LRCP.accept(listen_socket) do
      {:ok, %LRCP.Socket{} = socket} ->
        {:ok, handler} = Protohacker.LineReversal.Connection.start_link(socket)
        :ok = LRCP.controlling_process(socket, handler)

        accept_loop(listen_socket)

      {:error, reason} ->
        raise "failed to accept LRCP connection: #{inspect(reason)}"
    end
  end

# Then, from Protohacker.LineReversal.Connection
defmodule Protohacker.LineReversal.Connection do
  alias Protohacker.LineReversal.LRCP

  require Logger
  use GenServer

  def start_link(%LRCP.Socket{} = socket) do
    GenServer.start_link(__MODULE__, socket)
  end

  defstruct [
    :socket,
    buffer: <<>>
  ]

  @impl true
  def init(socket) do
    Logger.debug("connection started: #{inspect(socket)}")
    state = %__MODULE__{socket: socket}
    {:ok, state}
  end

  @impl true
  def handle_info({:lrcp, socket, data}, %__MODULE__{socket: socket} = state) do
    Logger.debug("received LRCP data: #{inspect(data)}")
    state = update_in(state.buffer, fn b -> b <> data end)
    state = handle_new_data(state)

    {:noreply, state}
  end

  def handle_info({:lrcp_error, socket, reason}, %__MODULE__{socket: socket} = state) do
    Logger.error("closing connection due to error: #{inspect(reason)}")
    {:stop, :normal, state}
  end

  def handle_info({:lrcp_closed, socket}, %__MODULE__{socket: socket} = state) do
    Logger.debug("Connection closed")
    {:stop, :normal, state}
  end

  # ------------------------
  # Helpers
  # ------------------------
  def handle_new_data(%__MODULE__{} = state) do
    case String.split(state.buffer, "\n", parts: 2) do
      [line, rest] ->
        LRCP.send(state.socket, String.reverse(line) <> "\n")
        handle_new_data(put_in(state.buffer, rest))

      [_no_line_yet] ->
        state
    end
  end
end
```
...even though we’re running over **unreliable, connectionless UDP**.

The Elixir code achieves this **perfectly**.

---

### 🔑 Core Idea: **Per-Session GenServer = Virtual TCP Socket**

In Elixir:
- **`LRCP.listen/2`** = like `gen_tcp:listen/2`
- **`LRCP.accept/1`** = like `gen_tcp:accept/1`
- **`LRCP.Socket` (a GenServer)** = like a `gen_tcp` socket pid

Each `LRCP.Socket` GenServer:
- Maintains **session state** (recv/send positions, buffers, timers)

```elixir 
# In Protohacker.LineReversal.LRCP.Socket
  defmodule State do
    defstruct [
      :udp_socket,
      :peer_ip,
      :peer_port,
      :session_id,
      :controlling_process,
      :idle_timer_ref,
      in_position: 0,
      out_position: 0,
      acked_out_position: 0,
      pending_out_payload: <<>>,
      out_message_queue: :queue.new()
    ]
  end
```

- Handles **LRCP protocol logic** (ACKs, retransmit, escaping)

```elixir 
# Protohacker.LineReversal.LRCP.Socket
  defp udp_send(%State{} = state, data) do
    :ok = :gen_udp.send(state.udp_socket, state.peer_ip, state.peer_port, data)
  end
```

- Presents a **clean stream interface** to the app

This is **exactly** how `gen_tcp` works internally — except `gen_tcp` uses OS sockets, while `LRCP.Socket` uses UDP + LRCP.

---

### 🧱 How It Mimics `gen_tcp` — Layer by Layer

| Layer          | `gen_tcp` (TCP)                     | Elixir LRCP                                    |
| -------------- | ----------------------------------- | ---------------------------------------------- |
| **Transport**  | OS kernel (TCP)                     | `:gen_udp` + LRCP protocol                     |
| **Connection** | Kernel-managed                      | Session GenServer (`LRCP.Socket`)              |
| **Accept**     | `gen_tcp:accept/1` blocks until SYN | `LRCP.accept/1` blocks until `/connect/`       |
| **Read**       | `gen_tcp:recv/2` → returns bytes    | `handle_info({:lrcp, data})` → app gets bytes  |
| **Write**      | `gen_tcp:send/2` → kernel queues    | `LRCP.send/2` → GenServer queues + retransmits |
| **Close**      | `gen_tcp:close/1` → sends FIN       | `/close/` message + GenServer terminates       |

---

### 🔄 Key Abstraction Mechanisms

#### 1. **`accept/1` Blocks Until Connection**
- `LRCP.ListenSocket` maintains a **queue of waiting acceptors** (`accept_queue`)
- When `/connect/` arrives, it either:
  - **Replies immediately** if someone is waiting in `accept_queue`, or
  - **Creates a session GenServer** and puts it in `ready_sockets`
- This is **identical** to how `gen_tcp:accept/1` blocks until a SYN arrives.

#### 2. **Per-Session GenServer Encapsulates State**
Each `LRCP.Socket` GenServer holds:
```elixir
%{
  in_position: 0,        # next expected byte (like TCP recv window)
  out_position: 0,       # next send position
  acked_out_position: 0, # what client has ACKed
  pending_out_payload: <<>>, # unacknowledged data (for retransmit)
  idle_timer_ref: timer_ref
}
```
This is **exactly** the state a TCP socket tracks in the kernel.

#### 3. **App Sees Only Bytes — Not Packets**
The app **never sees LRCP messages**. It only gets:
```elixir
def handle_info({:lrcp, _socket, data}, state) do
  # `data` is unescaped, in-order bytes — just like TCP!
end
```
All packetization, ACKs, retransmit, escaping — **hidden**.

#### 4. **Reliability via Actor Mailbox**
- UDP packets → dispatched to session GenServer via `Registry`
- GenServer processes messages **in order** (mailbox = FIFO queue)
- Retransmit timers are `Process.send_after`
- No shared state → no locks → no races

This is **message-passing reliability**, not shared-memory.


## How LRCP present a stream interface to application 

Excellent question! The **"clean stream interface"** in the Elixir LRCP implementation is achieved through a combination of **abstraction**, **message routing**, and **OTP conventions**. Let’s break it down step by step.

---

### 🎯 Goal of the Stream Interface

The application (e.g., `Connection` GenServer) should **only deal with bytes**, not LRCP protocol details:

```elixir
def handle_info({:lrcp, _socket, data}, state) do
  # `data` is plain, unescaped, in-order bytes — like TCP!
  # No slashes, no ACKs, no positions — just "hello\n"
end

# Sending is equally simple:
LRCP.send(socket, "olleh\n")
```

This is **exactly** like using `gen_tcp`.

---

### 🔑 How It’s Achieved: 4 Key Mechanisms

#### 1. **Per-Session GenServer Encapsulates Protocol Logic**

The `LRCP.Socket` GenServer **owns all LRCP state**:
- `in_position`: next expected byte offset
- `recv_buffer`: unescaped bytes received so far
- `pending_out_payload`: unacknowledged outgoing data
- Timers for retransmit/idle

When a UDP packet arrives:
- `LRCP.ListenSocket` parses it
- Routes it to the correct `LRCP.Socket` GenServer via `Registry`
- The session GenServer **validates**, **unescapes**, and **buffers** the data

✅ **App never sees raw LRCP messages**.

---

#### 2. **App Receives Data via Standard OTP Messaging**

The session GenServer **sends data to the app** using a standard OTP message:

```elixir
# Inside LRCP.Socket.handle_cast/2 (on valid /data/)
send_or_queue_message(state, {:lrcp, %__MODULE__{pid: self()}, unescaped_data})
```

Where `send_or_queue_message` does:

```elixir
defp send_or_queue_message(%State{controlling_process: pid}, message) when pid != nil do
  Kernel.send(pid, message)  # ← Standard OTP message
  state
end
```

So the app receives:
```elixir
{:lrcp, socket_pid, "hello\n"}
```

This is **identical** to how `gen_tcp` sends:
```elixir
{:tcp, socket_pid, "hello\n"}
```

> 💡 **This is the core of the abstraction**: the app uses the **same pattern** as TCP.

---

#### 3. **Controlling Process Handoff**

When `LRCP.accept/1` returns a socket, the app becomes its **controlling process**:

```elixir
# In Acceptor
{:ok, handler} = Connection.start_link(socket)
:ok = LRCP.controlling_process(socket, handler)
```

This tells the session GenServer:  
> “Send all `{:lrcp, ...}` messages to `handler`.”

If the app hasn’t started yet, messages are **queued** (`out_message_queue`) and delivered later.

✅ **No race conditions** between packet arrival and app startup.

---

#### 4. **Sending is Abstracted via `LRCP.send/2`**

When the app calls:
```elixir
LRCP.send(socket, "olleh\n")
```

It’s a simple GenServer call:
```elixir
def send(%__MODULE__{} = socket, data) do
  GenServer.call(socket.pid, {:send, data})
end
```

The session GenServer:
- Buffers the data
- Sends `/data/.../` over UDP
- Handles retransmission if no ACK

✅ **App doesn’t manage ACKs, escaping, or chunking**.

---

### 🔄 Full Data Flow: From Network to App

#### Receiving Data
```
UDP Packet: "/data/123/0/hello\\/\n/"
       ↓
LRCP.ListenSocket (parses → {:data, 123, 0, "hello\\/\n/"})
       ↓
Routes to LRCP.Socket (session 123) via Registry
       ↓
LRCP.Socket:
  - Validates pos == in_position (0)
  - Unescapes → "hello/\n"
  - Updates in_position = 8
  - Sends ACK: "/ack/123/8/"
  - Sends to app: {:lrcp, socket, "hello/\n"}
       ↓
App's Connection GenServer:
  handle_info({:lrcp, socket, "hello/\n"}, state)
```

#### Sending Data
```
App: LRCP.send(socket, "ol/leh\n")
       ↓
LRCP.Socket GenServer:
  - Escapes → "ol\\/leh\n"
  - Sends "/data/123/0/ol\\/leh\n/"
  - Buffers for retransmit
       ↓
UDP Packet: "/data/123/0/ol\\/leh\n/"
```

---

### 🧩 Why This Feels Like `gen_tcp`

| Feature       | `gen_tcp`                      | `LRCP.Socket`                   |
| ------------- | ------------------------------ | ------------------------------- |
| **Receive**   | `{:tcp, socket, data}`         | `{:lrcp, socket, data}`         |
| **Send**      | `:gen_tcp.send(socket, data)`  | `LRCP.send(socket, data)`       |
| **Close**     | `:gen_tcp.close(socket)`       | Session terminates on `/close/` |
| **Error**     | `{:tcp_error, socket, reason}` | `{:lrcp_error, socket, reason}` |
| **Ownership** | Controlling process            | Controlling process             |

The **only difference** is the message tag (`:tcp` vs `:lrcp`) — everything else is identical.

---

### ✅ Summary: The Abstraction Layers

1. **UDP Layer**: Raw packets with LRCP framing
2. **Protocol Layer**: `LRCP.ListenSocket` parses and routes
3. **Session Layer**: `LRCP.Socket` GenServer handles reliability
4. **Stream Layer**: App sees only `{:lrcp, socket, data}`

This is a **textbook example of protocol layering** — and why the Elixir solution is so elegant.

If you implement the same pattern in Rust (actor per session + message-based I/O), you’ll get the same clean interface.

Let me know if you'd like to see how to replicate this exact pattern in Rust!