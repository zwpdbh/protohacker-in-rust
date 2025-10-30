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
