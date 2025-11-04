#[cfg(test)]
mod protocol_encode_decode {
    use protohacker_in_rust::Result;
    use protohacker_in_rust::gossipglomers::*;
    use serde_json;
    #[test]
    fn case01() -> Result<()> {
        // Test case 1
        let original = Message {
            src: "a".to_string(),
            dst: "b".to_string(),
            body: MessageBody {
                id: Some(56),
                message_type: MessageType::ReadOk {
                    value: 4,
                    in_reply_to: 123,
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
}
