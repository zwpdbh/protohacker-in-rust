// https://protohackers.com/problem/1
// This problem exercise on how to read tcp stream line by line.

use crate::Result;

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tracing::error;

#[derive(Deserialize, Debug)]
struct Request {
    #[allow(unused)]
    method: Method,
    number: f64,
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
enum Method {
    IsPrime,
}

#[derive(Serialize, Debug)]
struct Response {
    method: Method,
    prime: bool,
}

impl Response {
    fn new(prime: bool) -> Self {
        Self {
            method: Method::IsPrime,
            prime,
        }
    }
}

pub async fn handle_client(mut socket: TcpStream) -> Result<()> {
    let (input_stream, output_stream) = socket.split();
    let _ = handle_client_internal(input_stream, output_stream).await?;

    Ok(())
}

async fn handle_client_internal(
    input_stream: impl AsyncRead + Unpin,
    mut output_stream: impl AsyncWrite + Unpin,
) -> Result<()> {
    let input_stream = BufReader::new(input_stream);
    // review: read line from bytes stream
    let mut lines = input_stream.lines();
    while let Some(line) = lines.next_line().await? {
        // review: deserilize line into struct object
        match serde_json::from_str::<Request>(&line) {
            Ok(req) => {
                let response = Response::new(is_prime(req.number));
                // review: serialize struct into bytes
                let mut bytes = serde_json::to_vec(&response)?;
                bytes.push(b'\n');
                output_stream.write_all(&bytes).await?;
                output_stream.flush().await?;
            }
            Err(e) => {
                error!("malformed request: {}", e);
                output_stream.write_all(b"malformed\n").await?;

                break;
            }
        }
    }
    Ok(())
}

fn is_prime(n: f64) -> bool {
    // Handle non-integer values
    if n.fract() != 0.0 {
        return false;
    }

    // Convert to integer (safe since we've checked it's a whole number)
    let n_int = n as i64;

    // Handle numbers less than 2
    if n_int < 2 {
        return false;
    }

    // Handle small primes
    if n_int == 2 {
        return true;
    }

    // Even numbers greater than 2 are not prime
    if n_int % 2 == 0 {
        return false;
    }

    // Check odd divisors up to sqrt(n)
    let sqrt_n = (n_int as f64).sqrt() as i64;
    for i in (3..=sqrt_n).step_by(2) {
        if n_int % i == 0 {
            return false;
        }
    }

    true
}

#[cfg(test)]
mod tests {
    #![allow(unused)]
    use super::*;
    use anyhow::{Ok, Result};

    #[test]
    fn test_is_prime() {
        // Prime numbers
        assert_eq!(is_prime(2.0), true);
        assert_eq!(is_prime(3.0), true);
        assert_eq!(is_prime(5.0), true);
        assert_eq!(is_prime(7.0), true);
        assert_eq!(is_prime(11.0), true);
        assert_eq!(is_prime(17.0), true);

        // Non-prime numbers
        assert_eq!(is_prime(1.0), false);
        assert_eq!(is_prime(4.0), false);
        assert_eq!(is_prime(6.0), false);
        assert_eq!(is_prime(8.0), false);
        assert_eq!(is_prime(9.0), false);
        assert_eq!(is_prime(15.0), false);

        // Non-integer inputs
        assert_eq!(is_prime(3.5), false);
        assert_eq!(is_prime(4.1), false);
        assert_eq!(is_prime(-2.0), false);
        assert_eq!(is_prime(0.0), false);
        assert_eq!(is_prime(1.0), false);
    }

    #[tokio::test]
    async fn prime_time_test_malformed() {
        let input = "{}\n";
        let mut output: Vec<u8> = vec![];

        handle_client_internal(input.as_bytes(), &mut output)
            .await
            .expect("Failed to handle");

        assert_eq!(
            String::from("malformed\n"),
            String::from_utf8(output).unwrap()
        );
    }
}
