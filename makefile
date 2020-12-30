BIN_DEBUG = target/debug/cocom
BIN_RELEASE = target/release/cocom
CARGO = cargo
DOC_DIR = doc/

.PHONY: build run doc

build:
	@$(CARGO) build

run-dev: build
	@$(CARGO) run -- 192.53.103.108

run: build
	@$(CARGO) run --release -- 192.53.103.108

doc:
	@rm -rf $(DOC_DIR)
	@$(CARGO) doc -j 2 -v --offline --target-dir $(DOC_DIR) --open