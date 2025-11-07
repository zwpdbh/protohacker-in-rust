deploy: 
	fly deploy 

log:
	flyctl logs -a protohacker-in-rust

status:
	fly status


# == maelstrom commands == 
# Need to download maelstrom from https://github.com/jepsen-io/maelstrom/tree/main and put it the project root
serve:
	./maelstrom/maelstrom serve

run_echo:
	cargo build
	./maelstrom/maelstrom test -w echo --bin scripts/run_echo.sh --node-count 1 --time-limit 10

run_unique_ids:
	cargo build
	./maelstrom/maelstrom test -w unique-ids --bin scripts/run_unique_ids.sh --time-limit 30 --rate 1000 --node-count 3 --availability total --nemesis partition

# Single-Node Broadcast
run_broadcast_1: 
	cargo build
	./maelstrom/maelstrom test -w broadcast --bin scripts/run_broadcast.sh --node-count 1 --time-limit 20 --rate 10

# Multi-Node Broadcast
run_broadcast_b: 
	cargo build
	./maelstrom/maelstrom test -w broadcast --bin scripts/run_broadcast.sh --node-count 5 --time-limit 20 --rate 10 --rate 10

# Fault Tolerant Broadcast
run_broadcast_c: 
	cargo build
	./maelstrom/maelstrom test -w broadcast --bin scripts/run_broadcast.sh --node-count 5 --time-limit 20 --rate 10 --nemesis partition

# Efficient Broadcast, Part I
run_broadcast_d: 
	cargo build
	./maelstrom/maelstrom test -w broadcast --bin scripts/run_broadcast.sh --node-count 25 --time-limit 20 --rate 100 --latency 100

# Efficient Broadcast, Part II
run_broadcast_e: 
	cargo build
	./maelstrom/maelstrom test -w broadcast --bin scripts/run_broadcast.sh --node-count 25 --time-limit 20 --rate 100 --latency 100