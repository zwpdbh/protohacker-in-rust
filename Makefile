deploy: 
	fly deploy 

log:
	flyctl logs -a protohacker-in-rust

status:
	fly status


# == maelstrom commands == 
run_echo:
	./maelstrom/maelstrom test -w echo --bin demo/ruby/echo.rb --time-limit 5

run_show:
	./maelstrom/maelstrom serve