use super::node::*;
use super::protocol::Message;
use crate::Result;

/// Template method pattern
pub async fn run_with_node<N: Node>(mut node: N) -> Result<()> {
    let stdin = std::io::stdin();

    let deserializer = serde_json::Deserializer::from_reader(stdin.lock());
    let mut stream = deserializer.into_iter::<Message>();

    while let Some(result) = stream.next() {
        let msg = result?;
        let _ = node.handle_message(msg).await?;
    }

    Ok(())
}
