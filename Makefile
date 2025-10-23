deploy: 
	fly deploy 

log:
	flyctl logs -a protohacker-in-rust

status:
	fly status