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