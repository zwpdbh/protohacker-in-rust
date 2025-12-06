#![allow(unused)]
use crate::Result;

// Examples of how OCaml's features provide strategic advantages for DAG workflow systems

// 1. Using functors for creating type-safe workflow components
// In OCaml this might look like:
/*
module type WORKFLOW_NODE = sig
  type input
  type output
  val process : input -> output
end

module type WORKFLOW_GRAPH = sig
  type node_id
  type dag
  val add_node : node_id -> (module WORKFLOW_NODE) -> dag -> dag
  val run : dag -> unit
end

module MakeWorkflow(Dep : SCHEDULER) : WORKFLOW_GRAPH = struct
  type node_id = string
  type dag = Dep.graph_type
  ...
end
*/

// Rust equivalent using trait bounds and associated types
trait WorkflowNode {
    type Input;
    type Output;

    fn process(&self, input: Self::Input) -> Self::Output;
}

// DAG representation with type safety
struct WorkflowNodeInstance<Node: WorkflowNode> {
    node: Node,
    id: String,
}

// This is where OCaml's GADTs would provide more expressiveness
// In OCaml with GADTs:
/*
type _ workflow_node =
  | Node : ('input -> 'output) -> ('input, 'output) workflow_node
  | Compose : ('a, 'b) workflow_node * ('b, 'c) workflow_node -> ('a, 'c) workflow_node

type dag = {
  nodes : 'a 'b. ('a, 'b) workflow_node list;
  dependencies : (int * int list) list;  (* node_id -> [depends_on...] *)
}
*/

// Rust equivalent - more complex due to type system limitations for heterogeneous collections
struct Dag<NodeType>
where
    NodeType: WorkflowNode,
{
    nodes: Vec<WorkflowNodeInstance<NodeType>>,
    dependencies: Vec<(usize, Vec<usize>)>, // node_idx -> [depends_on_idx...]
}

// OCaml's advantages for DAG workflows include:

// 1. First-class modules (functors) for dependency injection
// 2. GADTs for type-safe heterogeneous collections
// 3. More concise syntax for complex type relationships
// 4. Powerful pattern matching for workflow state management
// 5. Module system for organizing complex workflows

// Example of how OCaml might handle heterogeneous node types more elegantly:
/*
module type NodeSpec = sig
  type input
  type output
  val name : string
  val process : input -> output
end

module MakeTypedDAG(Node : NodeSpec) = struct
  type input_type = Node.input
  type output_type = Node.output
  type dag_node = (module NodeSpec with type input = input_type and type output = output_type)

  type dag = dag_node list
end
*/

// While Rust would use traits and associated types, OCaml's approach might be more direct
// for certain DAG patterns, especially where you need heterogeneous collections of
// differently-typed but workflow-related components.

// Let's look at a more complex example that shows the difference:

// In Rust, you'd need to use enums or trait objects for heterogeneous nodes
enum AnyWorkflowNode {
    IntToString(Box<dyn Fn(i32) -> String>),
    StringToInt(Box<dyn Fn(String) -> i32>),
    IntToInt(Box<dyn Fn(i32) -> i32>),
}

impl AnyWorkflowNode {
    fn execute_i32_to_string(&self, input: i32) -> Option<String> {
        match self {
            AnyWorkflowNode::IntToString(f) => Some(f(input)),
            _ => None,
        }
    }

    fn execute_string_to_i32(&self, input: String) -> Option<i32> {
        match self {
            AnyWorkflowNode::StringToInt(f) => Some(f(input)),
            _ => None,
        }
    }
}

// In OCaml with GADTs, this would be much more elegant:
/*
type _ node_type =
  | IntToString : (int -> string) -> int node_type
  | StringToInt : (string -> int) -> string node_type
  | IntToInt : (int -> int) -> int node_type

type dag_node = Node : 'a. 'a node_type
type dag = dag_node list
*/

// The key advantages of OCaml for DAG workflows:
// - GADTs allow type-safe heterogeneous collections
// - First-class modules (functors) enable clean dependency injection
// - More flexible type system for complex relationships
// - Pattern matching simplifies workflow state management
// - Module system helps organize large workflow systems

#[cfg(test)]
mod tests {
    use super::*;

    // Simple example of a workflow node
    struct IntegerProcessor;

    impl WorkflowNode for IntegerProcessor {
        type Input = i32;
        type Output = String;

        fn process(&self, input: Self::Input) -> Self::Output {
            format!("Processed: {}", input)
        }
    }

    #[test]
    fn example_workflow() -> Result<()> {
        let node = WorkflowNodeInstance {
            node: IntegerProcessor,
            id: "test_node".to_string(),
        };

        // Process input
        let result = node.node.process(42);
        println!("Result: {}", result);

        Ok(())
    }

    #[test]
    fn heterogeneous_example() -> Result<()> {
        let nodes = vec![
            AnyWorkflowNode::IntToString(Box::new(|x| format!("num: {}", x))),
            AnyWorkflowNode::StringToInt(Box::new(|s| s.len() as i32)),
        ];

        // Process through the workflow
        let input = 42;
        if let Some(transformed) = &nodes.get(0).and_then(|node| node.execute_i32_to_string(input)) {
            println!("Transformed: {}", transformed);
            if let Some(final_result) = &nodes.get(1).and_then(|node| node.execute_string_to_i32(transformed.clone())) {
                println!("Final result: {}", final_result);
            }
        }

        Ok(())
    }
}