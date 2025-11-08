# Efficient Gossip 

## Current situation 

- Gossip is triggered by a time ticker about 300ms interval. 
- Every gossip tick, each node sends all message it has received that haven't been gossiped to its neighbors.
  - This is tracked by (message_id, node_id) tuples in a HashSet.

## Things to try 

- Instead of sending all un-gossiped message, limit the number of messages sent in each gossip round. 
- Improve tracking pairs, use `HashMap<NodeId, hashSet<MessageId>>` which is map from `node_id` to set of messages that need to be gossiped to that node.
- Implement probabilistic gossip 
  - Randomly select a subset of neighbors
  - What is epidemic algorithms related with gossip?