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