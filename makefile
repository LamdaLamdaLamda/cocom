BIN_DEBUG = target/debug/cocom
BIN_RELEASE = target/release/cocom
CARGO = cargo
DOC_DIR = doc/
PREFIX = /usr/local
MODE = 711

.PHONY: build run run-dev install doc

build:
	@$(CARGO) build

run-dev: build
	@$(CARGO) run -- 192.53.103.108

run: build
	@$(CARGO) run --release -- 192.53.103.108

install: build
	@install -v -b -S .bak -m $(MODE) $(BIN_RELEASE) $(PREFIX)/bin

doc:
	@rm -rf $(DOC_DIR)
	@$(CARGO) doc -j 2 -v --offline --target-dir $(DOC_DIR) --open