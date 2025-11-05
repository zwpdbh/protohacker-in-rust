use super::protocol::Message;
use crate::{Result, maelstrom::nodes::unique_ids_node::UniqueIdsNode};

pub fn run() -> Result<()> {
    // Maelstrom works with any kind of binary, feeding it network messages on stdin
    let reader = std::io::stdin().lock();

    // receiving network messages from stdout
    // and logging information on stderr.
    let mut stdout = std::io::stdout().lock();

    let inputs = serde_json::Deserializer::from_reader(reader).into_iter::<Message>();

    let mut state = UniqueIdsNode::new();

    for input in inputs {
        let msg = input?;
        let _ = state.handle(msg, &mut stdout)?;
    }

    Ok(())
}
