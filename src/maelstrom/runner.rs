use super::node::*;
use super::protocol::Message;
use crate::Result;

/// Template method pattern
pub fn run_with_node<N: Node>(mut node: N) -> Result<()> {
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout().lock();

    let deserializer = serde_json::Deserializer::from_reader(stdin.lock());
    let mut stream = deserializer.into_iter::<Message>();

    while let Some(result) = stream.next() {
        let msg = result?;
        if !node.handle_message(msg.clone(), &mut stdout)? {
            eprintln!("Unhandled message: {:?}", msg.body.payload);
        }
    }

    Ok(())
}
