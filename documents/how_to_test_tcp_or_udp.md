# How to test tcp or udp 

When writing server for tcp or udp, how to test the core logic of our service.

## TCP 

Instead of test the TCP layer: 

1. Extract the core logic into a generic function 

```rust 
async fn handle_client_internal<I, O>(
    room: Room,
    client_id: ClientId,
    mut sink: O,
    mut stream: I,
) -> Result<()>
where
    I: Stream<Item = Result<String>> + Unpin,
    O: Sink<OutgoingMessage, Error = Error> + Unpin,
{
    // Protocol logic here (welcome, login, chat, etc.)
}
```

Decoupled from TCP. This function doesn't know it is talking to a TCP socket. It only cares that: 
- It can receive Result<String> items (via Stream)
- It can send OutgoingMessage items (via Sink)

Only depends on async I/O traits.

2. In Production 

We pass it real TCP havles: 

```rust 
let framed = Framed::new(tcp_stream, ChatCodec::new());
let (input_stream, output_stream) = framed.split();
handle_client_internal(room, client_id, output_stream, input_stream).await?;
```

3. In Test 

- Create `stream` from `mpsc::channel`.
- Create `sink` from `PollSender`.
- Mimic user connect by returning struct which could receive from `sink` 
  and send message to `Stream`
  
```rust  
    struct UserTest {
        sink_receiver: Receiver<OutgoingMessage>,
        stream_sender: Option<Sender<Result<String>>>,
        handle: JoinHandle<Result<()>>,
    }

    async fn connect(room: Room, client_id: ClientId) -> UserTest {
        let (sink_tx, sink_rx) = mpsc::channel(100);

        let (stream_tx, mut stream_rx) = mpsc::channel(100);

        // create stream from channel
        let stream = async_stream::stream! {
            while let Some(message) = stream_rx.recv().await {
                yield message
            }
        };

        // make sender compatible with `Sink` trait
        let sink = PollSender::new(sink_tx).sink_map_err(|e| Error::General(e.to_string()));

        let handle = tokio::spawn(async move {
            handle_client_internal(room, client_id, sink, Box::pin(stream)).await
        });

        UserTest {
            sink_receiver: sink_rx,
            stream_sender: Some(stream_tx),
            handle,
        }
    }

    impl UserTest {
        async fn send(&mut self, message: &str) {
            self.stream_sender
                .as_ref()
                .unwrap()
                .send(Ok(message.to_string()))
                .await
                .unwrap();
        }

        async fn leave(mut self) {
            let stream = self.stream_sender.take();
            drop(stream);

            self.handle.await.unwrap().unwrap()
        }

        async fn check_message(&mut self, msg: OutgoingMessage) {
            assert_eq!(self.sink_receiver.recv().await.unwrap(), msg);
        }
    }
```

## UDP 

Unit test UDP is generally easier than TCP, because it is message-oriented and stateless per packet.
There’s no connection lifecycle, no stream framing, and no need to split I/O halves.

For simple case
- we could seperate business function into `async fn handle_message(db: &mut Db, payload: &[u8]) -> Option<Vec<u8>>`.
- Then use it as: 
  
```rust 
if let Some(resonse) = handle_message(&mut db, payload).await {
    socket.send_to(&resonse, src_addr).await?;
}
```


## Summary 

- In general, we need to find a way to make handle client connection independent of `socket`.
- In tcp, we first split the `socket` into `TcpStream`. Therefore. the `handle_client_internal` need to use trait which make `TcpStream` compatible.
- In udp, we could just test on function `async fn handle_message(db: &mut Db, payload: &[u8]) -> Option<Vec<u8>>`.
