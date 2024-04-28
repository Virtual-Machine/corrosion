run:
	cargo run

run-test:
	cargo run --features "test-suite"

run-debug:
	cargo run --features "debug-full test-suite"

run-all:
	cargo run --features "debug-full test-suite test-block-write"