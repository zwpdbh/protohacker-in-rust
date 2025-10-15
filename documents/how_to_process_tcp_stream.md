# How to process TCP stream 

## Use Codec 

In the context of `tokio_util:codec`:
- Frame = A transport-level unit of data. Means "This is one complete unit of bytes from the stream"
  - Lines 
  - Length-prefixed chunks.
  - HTTP chunks 
- Message = An application-level semantic unit 
  - Usually obtained by parsing or deserializing a frame. 

```txt 
TCP Stream (bytes)
       ↓
[Decoder] → produces **frames** (e.g., String, Bytes)
       ↓
[Parser / Deserializer] → produces **messages** (e.g., Request, Command)
       ↓
Application Logic
```

1. Line Framing 

See problem 03.

2. Length-prefixd framing

if problem03's protocol is length-prefix frameing, it may looks like: 

```rust 
// lib.rs or main.rs
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum OutgoingMessage {
    Chat { from: String, text: String },
    UserJoin(String),
    UserLeave(String),
    Welcome,
    Participants(Vec<String>),
    InvalidUsername(String),
}

// For symmetry, you might also define:
#[derive(Serialize, Deserialize, Debug)]
pub enum IncomingMessage {
    Login(String),
    ChatText(String),
}
```

Add to Cargo.toml

```rust 
[dependencies]
tokio = { version = "1", features = ["full"] }
tokio-util = { version = "0.7", features = ["codec"] }
bytes = "1"
serde = { version = "1", features = ["derive"] }
bincode = "1"
serde_json = "1"
```

For bincode + 8-byte length prefix

```rust
use tokio_util::codec::{Decoder, Encoder, LengthDelimitedCodec, BigEndian};
use bytes::{BytesMut, Bytes};
use crate::{OutgoingMessage, IncomingMessage, Error, Result};

pub struct BincodeChatCodec {
    inner: LengthDelimitedCodec,
}

impl BincodeChatCodec {
    pub fn new() -> Self {
        Self {
            inner: LengthDelimitedCodec::builder()
                .length_field_length(8)               // u64
                .length_field_endianness(BigEndian)
                .max_frame_length(1024 * 1024)        // 1 MB max
                .new_codec(),
        }
    }
}

impl Encoder<OutgoingMessage> for BincodeChatCodec {
    type Error = Error;

    fn encode(&mut self, item: OutgoingMessage, dst: &mut BytesMut) -> Result<()> {
        let encoded = bincode::serialize(&item)
            .map_err(|e| Error::General(format!("Bincode encode error: {}", e)))?;
        self.inner.encode(Bytes::from(encoded), dst)
            .map_err(|e| Error::General(e.to_string()))
    }
}

impl Decoder for BincodeChatCodec {
    type Item = IncomingMessage;
    type Error = Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>> {
        match self.inner.decode(src)? {
            Some(bytes) => {
                let msg = bincode::deserialize(&bytes)
                    .map_err(|e| Error::General(format!("Bincode decode error: {}", e)))?;
                Ok(Some(msg))
            }
            None => Ok(None),
        }
    }
}
```

For serde_json + 8-byte length prefix

```rust 
use tokio_util::codec::{Decoder, Encoder, LengthDelimitedCodec, BigEndian};
use bytes::{BytesMut, Bytes};
use crate::{OutgoingMessage, IncomingMessage, Error, Result};

pub struct JsonChatCodec {
    inner: LengthDelimitedCodec,
}

impl JsonChatCodec {
    pub fn new() -> Self {
        Self {
            inner: LengthDelimitedCodec::builder()
                .length_field_length(8)               // u64
                .length_field_endianness(BigEndian)
                .max_frame_length(1024 * 1024)        // 1 MB max
                .new_codec(),
        }
    }
}

impl Encoder<OutgoingMessage> for JsonChatCodec {
    type Error = Error;

    fn encode(&mut self, item: OutgoingMessage, dst: &mut BytesMut) -> Result<()> {
        let json = serde_json::to_vec(&item)
            .map_err(|e| Error::General(format!("JSON encode error: {}", e)))?;
        self.inner.encode(Bytes::from(json), dst)
            .map_err(|e| Error::General(e.to_string()))
    }
}

impl Decoder for JsonChatCodec {
    type Item = IncomingMessage;
    type Error = Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>> {
        match self.inner.decode(src)? {
            Some(bytes) => {
                let msg = serde_json::from_slice(&bytes)
                    .map_err(|e| Error::General(format!("JSON decode error: {}", e)))?;
                Ok(Some(msg))
            }
            None => Ok(None),
        }
    }
}
```

Usage

```rust 
use tokio::net::TcpStream;
use tokio_util::codec::Framed;

// For bincode
let stream = TcpStream::connect("127.0.0.1:8080").await?;
let mut framed: Framed<_, BincodeChatCodec> = Framed::new(stream, BincodeChatCodec::new());

framed.send(OutgoingMessage::Welcome).await?;

if let Some(Ok(msg)) = framed.next().await {
    // msg is IncomingMessage
}
```


## References

- [bincode](https://docs.rs/bincode/latest/bincode/)
- [tokio](https://tokio.rs/tokio/tutorial/framing)