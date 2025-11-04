#[cfg(test)]
mod line_reversal_tests {
    #[allow(unused)]
    use ::tracing::debug;
    use protohacker_in_rust::protohackers::problem7::{RETRANSMIT_MILLIS, run};
    use protohacker_in_rust::tracer;
    use protohacker_in_rust::{Error, Result};
    use std::time::Duration;
    use tokio::net::UdpSocket;
    use tokio::time::timeout;

    use std::sync::Once;

    const SESSION_ID: u64 = 12345;
    const SERVER_PORT: u32 = 3000;

    static TRACING: Once = Once::new();

    fn init_tracing() {
        TRACING.call_once(|| {
            let _x = tracer::setup_simple_tracing();
        });
    }

    async fn udp_send(socket: &UdpSocket, server_addr: &str, msg: &str) -> Result<()> {
        socket.send_to(msg.as_bytes(), server_addr).await?;

        Ok(())
    }

    async fn udp_recv(socket: &UdpSocket) -> Result<String> {
        let mut buf = [0; 1024];
        let (len, _) = timeout(Duration::from_secs(2), socket.recv_from(&mut buf))
            .await
            .map_err(|_| Error::Other("Timeout waiting for response".into()))??;

        let response = String::from_utf8_lossy(&buf[..len]).to_string();
        Ok(response)
    }

    #[tokio::test]
    /// Test example session from protohacker
    async fn test_line_reversal_session() -> Result<()> {
        let _x = init_tracing();

        // Start the server in the background
        let server_handle = tokio::spawn(async {
            if let Err(e) = run(SERVER_PORT).await {
                eprintln!("Server error: {:?}", e);
            }
        });

        let client_socket = UdpSocket::bind("127.0.0.1:0").await?;
        let server_addr = format!("127.0.0.1:{}", SERVER_PORT);

        // 1. Connect
        let connect_msg = format!("/connect/{SESSION_ID}/");
        let _ = udp_send(&client_socket, &server_addr, &connect_msg).await?;

        // let ack = send_and_recv(&client_socket, &server_addr, &connect_msg).await?;
        assert_eq!(
            udp_recv(&client_socket).await?,
            format!("/ack/{SESSION_ID}/0/")
        );

        // 2. Send "hello\n"
        let _ = udp_send(
            &client_socket,
            &server_addr,
            &format!("/data/{SESSION_ID}/0/hello\n/"),
        )
        .await?;
        // let ack1 = send_and_recv(&client_socket, &server_addr, &data1).await?;
        assert_eq!(
            udp_recv(&client_socket).await?,
            format!("/ack/{SESSION_ID}/6/",)
        );

        // 3. Expect reversed "olleh\n" from server
        assert_eq!(
            udp_recv(&client_socket).await?,
            format!("/data/{SESSION_ID}/0/olleh\n/")
        );

        // Ack the server's data
        let _ = udp_send(
            &client_socket,
            &server_addr,
            &format!("/ack/{SESSION_ID}/6/"),
        )
        .await?;

        // 4. Send "Hello, world!\n"
        let _ = udp_send(
            &client_socket,
            &server_addr,
            &format!("/data/{SESSION_ID}/6/Hello, world!\n/"),
        )
        .await?;

        assert_eq!(
            udp_recv(&client_socket).await?,
            format!("/ack/{SESSION_ID}/20/")
        );

        // 5. Expect reversed "!dlrow ,olleH\n"
        assert_eq!(
            udp_recv(&client_socket).await?,
            format!("/data/{SESSION_ID}/6/!dlrow ,olleH\n/")
        );

        // Ack it
        let _ = udp_send(
            &client_socket,
            &server_addr,
            &format!("/ack/{SESSION_ID}/20/"),
        )
        .await?;

        // 6. Close session
        let _ = udp_send(
            &client_socket,
            &server_addr,
            &format!("/close/{SESSION_ID}/"),
        )
        .await?;

        assert_eq!(
            udp_recv(&client_socket).await?,
            format!("/close/{SESSION_ID}/")
        );

        // Cancel server (it's designed to run forever, so we just drop the task)
        server_handle.abort();

        Ok(())
    }

    #[tokio::test]
    /// Test when received data without containing new line character and then received a new line character
    /// It should still return the reversed whole line
    async fn test_sent_broken_packets() -> Result<()> {
        let _x = init_tracing();

        // Start the server in the background
        let server_handle = tokio::spawn(async {
            if let Err(e) = run(SERVER_PORT).await {
                eprintln!("Server error: {:?}", e);
            }
        });

        let client_socket = UdpSocket::bind("127.0.0.1:0").await?;
        let server_addr = format!("127.0.0.1:{}", SERVER_PORT);

        // 1. Connect
        let _ = udp_send(
            &client_socket,
            &server_addr,
            &format!("/connect/{SESSION_ID}/"),
        )
        .await?;

        assert_eq!(
            udp_recv(&client_socket).await?,
            format!("/ack/{SESSION_ID}/0/")
        );

        // 2. Send "hello "
        let _ = udp_send(
            &client_socket,
            &server_addr,
            &format!("/data/{SESSION_ID}/0/hello /"),
        )
        .await?;

        assert_eq!(
            udp_recv(&client_socket).await?,
            format!("/ack/{SESSION_ID}/6/",)
        );

        // 3. Send "world!"

        let _ = udp_send(
            &client_socket,
            &server_addr,
            &format!("/data/{SESSION_ID}/6/world!/"),
        )
        .await?;
        assert_eq!(
            udp_recv(&client_socket).await?,
            format!("/ack/{SESSION_ID}/12/",)
        );

        let _ = udp_send(
            &client_socket,
            &server_addr,
            &format!("/data/{SESSION_ID}/12/\\/\n/"),
        )
        .await?;
        assert_eq!(
            udp_recv(&client_socket).await?,
            format!("/ack/{SESSION_ID}/14/",)
        );

        assert_eq!(
            udp_recv(&client_socket).await?,
            format!(
                "/data/12345/0/\\/{}/",
                "hello world!".chars().rev().collect::<String>() + "\n"
            ),
        );

        // Cancel server (it's designed to run forever, so we just drop the task)
        server_handle.abort();

        Ok(())
    }

    #[tokio::test]
    /// Test server retrasmits data if it doesn't receive acks
    async fn test_retransmit() -> Result<()> {
        let _x = init_tracing();

        // Start the server in the background
        let server_handle = tokio::spawn(async {
            if let Err(e) = run(SERVER_PORT).await {
                eprintln!("Server error: {:?}", e);
            }
        });

        let client_socket = UdpSocket::bind("127.0.0.1:0").await?;
        let server_addr = format!("127.0.0.1:{}", SERVER_PORT);

        // 1. Connect
        let _ = udp_send(
            &client_socket,
            &server_addr,
            &format!("/connect/{SESSION_ID}/"),
        )
        .await?;
        assert_eq!(
            udp_recv(&client_socket).await?,
            format!("/ack/{SESSION_ID}/0/")
        );

        // 2. client send "hello "
        let _ = udp_send(
            &client_socket,
            &server_addr,
            &format!("/data/{SESSION_ID}/0/hello\n/"),
        )
        .await?;

        // client first receive ack indicate the server has received all.
        assert_eq!(
            udp_recv(&client_socket).await?,
            format!("/ack/{SESSION_ID}/6/",)
        );

        // --------------
        // should resend if doesn't receive acks
        // --------------

        // client receive reversed result, but doesn't ack it.
        assert_eq!(
            udp_recv(&client_socket).await?,
            format!(
                "/data/{SESSION_ID}/0/{}/",
                "hello".chars().rev().collect::<String>() + "\n"
            )
        );

        // after retransmit interval, client should receive retransmitted reversed result.
        let _ = tokio::time::sleep(Duration::from_millis(RETRANSMIT_MILLIS as u64)).await;
        assert_eq!(
            udp_recv(&client_socket).await?,
            format!(
                "/data/{SESSION_ID}/0/{}/",
                "hello".chars().rev().collect::<String>() + "\n"
            )
        );

        let _ = tokio::time::sleep(Duration::from_millis(RETRANSMIT_MILLIS as u64)).await;
        assert_eq!(
            udp_recv(&client_socket).await?,
            format!(
                "/data/{SESSION_ID}/0/{}/",
                "hello".chars().rev().collect::<String>() + "\n"
            )
        );

        // client Ack reversed result but only partial of it
        let _ = udp_send(
            &client_socket,
            &server_addr,
            &format!("/ack/{SESSION_ID}/3/"),
        )
        .await?;

        // client received rest
        assert_eq!(
            udp_recv(&client_socket).await?,
            format!("/data/{SESSION_ID}/3/{}/", "eh\n")
        );

        let _ = tokio::time::sleep(Duration::from_millis(RETRANSMIT_MILLIS as u64)).await;
        assert_eq!(
            udp_recv(&client_socket).await?,
            format!("/data/{SESSION_ID}/3/{}/", "eh\n")
        );

        server_handle.abort();

        Ok(())
    }

    #[tokio::test]

    async fn test_close_if_client_misbehaves() -> Result<()> {
        let _x = init_tracing();

        // Start the server in the background
        let server_handle = tokio::spawn(async {
            if let Err(e) = run(SERVER_PORT).await {
                eprintln!("Server error: {:?}", e);
            }
        });

        let client_socket = UdpSocket::bind("127.0.0.1:0").await?;
        let server_addr = format!("127.0.0.1:{}", SERVER_PORT);

        // 1. Connect
        let _ = udp_send(
            &client_socket,
            &server_addr,
            &format!("/connect/{SESSION_ID}/"),
        )
        .await?;
        assert_eq!(
            udp_recv(&client_socket).await?,
            format!("/ack/{SESSION_ID}/0/")
        );

        let message = "either/or\n";
        let msg_len = message.len();

        let _ = udp_send(
            &client_socket,
            &server_addr,
            &format!("/data/{SESSION_ID}/0/{}/", "either\\/or\n"),
        )
        .await?;

        assert_eq!(
            udp_recv(&client_socket).await?,
            format!("/ack/{SESSION_ID}/{msg_len}/")
        );
        assert_eq!(
            udp_recv(&client_socket).await?,
            format!("/data/{SESSION_ID}/0/ro\\/rehtie\n/")
        );

        // Ack only two bytes, which means that the server should resend us the rest of the data.
        let _ = udp_send(
            &client_socket,
            &server_addr,
            &format!("/ack/{SESSION_ID}/2/"),
        )
        .await?;
        assert_eq!(
            udp_recv(&client_socket).await?,
            format!("/data/{SESSION_ID}/2/\\/rehtie\n/")
        );

        //  Ack another two bytes and let the server resend the rest.
        let _ = udp_send(
            &client_socket,
            &server_addr,
            &format!("/ack/{SESSION_ID}/3/"),
        )
        .await?;
        assert_eq!(
            udp_recv(&client_socket).await?,
            format!("/data/{SESSION_ID}/3/rehtie\n/")
        );

        //  # If we ack something we already acked again, nothing happens.
        let _ = udp_send(
            &client_socket,
            &server_addr,
            &format!("/ack/{SESSION_ID}/1/"),
        )
        .await?;

        // If we hack further than what the server sent us, it's a protocol error.
        let _ = udp_send(
            &client_socket,
            &server_addr,
            &format!("/ack/{SESSION_ID}/1000/"),
        )
        .await?;

        assert_eq!(
            udp_recv(&client_socket).await?,
            format!("/close/{SESSION_ID}/")
        );

        server_handle.abort();

        Ok(())
    }
}
