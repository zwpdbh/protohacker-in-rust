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