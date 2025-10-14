use std::{collections::BTreeMap, io::ErrorKind, ops::RangeInclusive};

use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};

use crate::{Error, Result};

struct Db(BTreeMap<i32, i32>);

impl Db {
    fn new() -> Db {
        Db(BTreeMap::new())
    }

    fn insert(&mut self, timestamp: i32, price: i32) {
        self.0.insert(timestamp, price);
    }

    pub fn mean(&self, range: RangeInclusive<i32>) -> i32 {
        if range.is_empty() {
            return 0;
        };
        let (count, sum) = self
            .0
            .range(range)
            .fold((0, 0_i64), |(count, sum), (_index, v)| {
                (count + 1, sum + *v as i64)
            });

        if count > 0 { (sum / count) as i32 } else { 0 }
    }
}

pub async fn run(port: u32) -> Result<()> {
    let address = format!("127.0.0.1:{port}");
    let listener = TcpListener::bind(address.clone()).await?;
    loop {
        let (socket, _addr) = listener.accept().await?;
        tokio::spawn(handle_client(socket));
    }
}

async fn handle_client(mut socket: TcpStream) -> Result<()> {
    let (input_stream, output_stream) = socket.split();
    let _ = handle_client_internal(input_stream, output_stream).await?;

    Ok(())
}

async fn handle_client_internal(
    mut input_stream: impl AsyncRead + Unpin,
    mut output_stream: impl AsyncWrite + Unpin,
) -> Result<()> {
    let mut db = Db::new();
    let mut buffer = [0u8; 9];

    loop {
        // review: read from stream for exact n bytes
        match input_stream.read_exact(&mut buffer).await {
            Ok(_n) => {
                let message = Message::parse(&buffer)?;
                match message {
                    Message::Insert { timestamp, price } => {
                        db.insert(timestamp, price);
                    }
                    Message::Query { mintime, maxtime } => {
                        let mean = db.mean(mintime..=maxtime);
                        output_stream.write_all(&mean.to_be_bytes()).await?;
                    }
                }
            }
            Err(e) if e.kind() == ErrorKind::UnexpectedEof => return Ok(()),
            Err(e) => return Err(Error::Io(e)),
        }
    }
}

#[derive(Debug, PartialEq)]
enum Message {
    Insert { price: i32, timestamp: i32 },
    Query { mintime: i32, maxtime: i32 },
}

impl Message {
    // parse specific n bytes
    pub fn parse(buffer: &[u8; 9]) -> Result<Self> {
        let op = buffer[0];
        let first = i32::from_be_bytes(buffer[1..5].try_into()?);
        let second = i32::from_be_bytes(buffer[5..9].try_into()?);

        match op {
            b'I' => Ok(Message::Insert {
                price: second,
                timestamp: first,
            }),
            b'Q' => Ok(Message::Query {
                mintime: first,
                maxtime: second,
            }),
            _ => Err(Error::InvalidProtocol(format!("unexpected op code {}", op))),
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(unused)]
    use super::*;
    use anyhow::{Ok, Result};

    #[test]
    fn message_encode() -> Result<()> {
        // Test case 1
        Ok(())
    }

    async fn create_message(op: u8, first: i32, second: i32) -> [u8; 9] {
        let mut buffer = vec![];
        buffer.write_u8(op).await.unwrap();
        buffer.write_i32(first).await.unwrap();
        buffer.write_i32(second).await.unwrap();

        buffer.try_into().unwrap()
    }

    #[tokio::test]
    async fn message_parse_test_insert_ok() {
        let buffer = create_message(b'I', 10, 100).await;

        let message = Message::parse(&buffer).unwrap();

        assert_eq!(
            Message::Insert {
                timestamp: 10,
                price: 100
            },
            message
        )
    }

    #[tokio::test]
    async fn message_parse_test_query_ok() {
        let buffer = create_message(b'Q', 10, 100).await;

        let message = Message::parse(&buffer).unwrap();

        assert_eq!(
            Message::Query {
                mintime: 10,
                maxtime: 100,
            },
            message
        )
    }

    #[tokio::test]
    async fn example_session_test() {
        let messages = vec![
            create_message(b'I', 12345, 101).await,
            create_message(b'I', 123456, 102).await,
            create_message(b'I', 123456, 100).await,
            create_message(b'I', 40960, 5).await,
            create_message(b'Q', 12288, 16384).await,
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<u8>>();

        let mut output = vec![];

        handle_client_internal(messages.as_slice(), &mut output)
            .await
            .unwrap();

        assert_eq!(4, output.len());
        assert_eq!(101, i32::from_be_bytes(output[..4].try_into().unwrap()));
    }
}
