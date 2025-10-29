#[cfg(test)]
mod line_reversal_tests {
    #[allow(unused)]
    use ::tracing::debug;
    use protohacker_in_rust::protohackers::problem7;
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

    async fn send_and_recv(socket: &UdpSocket, server_addr: &str, msg: &str) -> Result<String> {
        socket.send_to(msg.as_bytes(), server_addr).await?;

        let mut buf = [0; 1024];
        let (len, _) = timeout(Duration::from_secs(2), socket.recv_from(&mut buf))
            .await
            .map_err(|_| Error::Other("Timeout waiting for response".into()))??;

        let response = String::from_utf8_lossy(&buf[..len]).to_string();
        Ok(response)
    }

    #[tokio::test]
    async fn test_line_reversal_session() -> Result<()> {
        let _x = init_tracing();

        // Start the server in the background
        let server_handle = tokio::spawn(async {
            if let Err(e) = problem7::run(SERVER_PORT).await {
                eprintln!("Server error: {:?}", e);
            }
        });

        let client_socket = UdpSocket::bind("127.0.0.1:0").await?;
        let server_addr = format!("127.0.0.1:{}", SERVER_PORT);

        // 1. Connect
        let connect_msg = format!("/connect/{SESSION_ID}/");
        let ack = send_and_recv(&client_socket, &server_addr, &connect_msg).await?;
        assert_eq!(ack, format!("/ack/{SESSION_ID}/0/"));

        // 2. Send "hello\n"
        let data1 = format!("/data/{SESSION_ID}/0/hello\n/");
        let ack1 = send_and_recv(&client_socket, &server_addr, &data1).await?;
        assert_eq!(ack1, format!("/ack/{SESSION_ID}/6/",));

        // 3. Expect reversed "olleh\n" from server
        let mut buf = [0; 1024];
        let (len, _) = timeout(Duration::from_secs(2), client_socket.recv_from(&mut buf))
            .await
            .map_err(|_| Error::Other("Timeout waiting for server data".into()))??;
        let server_data1 = String::from_utf8_lossy(&buf[..len]).to_string();
        // Server sends: /data/12345/0/olleh\n/
        // Note: \n is literal newline, so escaped as \\n in string
        assert_eq!(server_data1, format!("/data/{SESSION_ID}/0/olleh\n/"));

        // Ack the server's data
        let ack_server_data = format!("/ack/{SESSION_ID}/6/");
        client_socket
            .send_to(ack_server_data.as_bytes(), &server_addr)
            .await?;

        // 4. Send "Hello, world!\n"
        let data2 = format!("/data/{SESSION_ID}/6/Hello, world!\n/");
        let ack2 = send_and_recv(&client_socket, &server_addr, &data2).await?;
        assert_eq!(ack2, format!("/ack/{SESSION_ID}/20/"));

        // 5. Expect reversed "!dlrow ,olleH\n"
        let (len, _) = timeout(Duration::from_secs(2), client_socket.recv_from(&mut buf))
            .await
            .map_err(|_| Error::Other("Timeout waiting for second server data".into()))??;

        let server_data2 = String::from_utf8_lossy(&buf[..len]).to_string();
        assert_eq!(
            server_data2,
            format!("/data/{SESSION_ID}/6/!dlrow ,olleH\n/")
        );

        // Ack it
        let ack_server_data2 = format!("/ack/{SESSION_ID}/20/",);
        client_socket
            .send_to(ack_server_data2.as_bytes(), &server_addr)
            .await?;

        // 6. Close session
        let close_msg = format!("/close/{SESSION_ID}/");
        let close_resp = send_and_recv(&client_socket, &server_addr, &close_msg).await?;
        assert_eq!(close_resp, format!("/close/{SESSION_ID}/"));

        // Cancel server (it's designed to run forever, so we just drop the task)
        server_handle.abort();

        Ok(())
    }
}
