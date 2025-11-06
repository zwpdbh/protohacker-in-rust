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

run_broadcast: 
	cargo build
	./maelstrom/maelstrom test -w broadcast --bin scripts/run_broadcast.sh --time-limit 30 --node-count 1 --time-limit 20 --rate 10