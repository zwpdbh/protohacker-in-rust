#!/bin/bash
# Navigate to the directory where this script resides, then to the project root
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"  # Adjust if needed (e.g., if script is deeper)

cd "$PROJECT_ROOT" || exit 1

# exec replaces the current shell process with your Rust binary.
exec ./target/debug/protohacker-in-rust maelstrom echo