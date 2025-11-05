#[cfg(test)]
mod protocol_encode_decode {
    use protohacker_in_rust::Result;
    use protohacker_in_rust::maelstrom::*;
    use serde_json;
    #[test]
    fn case01() -> Result<()> {
        // Test case 1
        let original = Message {
            src: "a".to_string(),
            dst: "b".to_string(),
            body: MessageBody {
                id: Some(56),
                payload: Payload::ReadOk {
                    value: 4,
                    in_reply_to: Some(123),
                },
            },
        };

        // Serialize to JSON
        let json = serde_json::to_string(&original)?;

        // Deserialize back
        let decoded: Message = serde_json::from_str(&json)?;

        // Assert round-trip equality
        assert_eq!(original, decoded);

        // Optional: assert against expected JSON string (for strict wire format check)
        let expected_json = r#"{"src":"a","dest":"b","body":{"msg_id":56,"type":"read_ok","value":4,"in_reply_to":123}}"#;
        assert_eq!(json, expected_json);

        Ok(())
    }

    #[test]
    fn case02_echo_round_trip() -> Result<()> {
        // Incoming echo message from Maelstrom
        let input_json = r#"{
            "src": "c1",
            "dest": "n1",
            "body": {
                "type": "echo",
                "msg_id": 1,
                "echo": "Please echo 35"
            }
        }"#;

        // Deserialize
        let msg: Message = serde_json::from_str(input_json)?;

        // Validate deserialized content
        assert_eq!(msg.src, "c1");
        assert_eq!(msg.dst, "n1");
        assert_eq!(msg.body.id, Some(1));
        match msg.body.payload {
            Payload::Echo { ref echo } => {
                assert_eq!(echo, "Please echo 35");
            }
            _ => panic!("Expected Echo variant"),
        }

        // Construct reply
        let reply = Message {
            src: "n1".to_string(),
            dst: "c1".to_string(),
            body: MessageBody {
                id: Some(2), // new msg_id for reply (Maelstrom usually assigns this, but we can simulate)
                payload: Payload::EchoOk {
                    echo: "Please echo 35".to_string(),
                    in_reply_to: Some(1),
                },
            },
        };

        // Serialize reply
        let reply_json = serde_json::to_string(&reply)?;
        let expected_reply = r#"{"src":"n1","dest":"c1","body":{"msg_id":2,"type":"echo_ok","echo":"Please echo 35","in_reply_to":1}}"#;

        assert_eq!(reply_json, expected_reply);

        // Also test round-trip deserialization of reply
        let decoded_reply: Message = serde_json::from_str(&reply_json)?;
        assert_eq!(reply, decoded_reply);

        Ok(())
    }
}
