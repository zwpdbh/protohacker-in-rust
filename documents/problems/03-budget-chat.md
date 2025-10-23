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
