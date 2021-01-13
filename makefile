BIN_DEBUG = target/debug/cocom
BIN_RELEASE = target/release/cocom
UNIT_TEST_NTP = test_ntp
UNIT_TEST_CLIENT = test_client
CARGO = cargo
DOC_DIR = doc/
PREFIX = /usr/local
MODE = 711

.PHONY: build build-dev run run-dev install doc test

build-dev:
	@$(CARGO) build

build:
	@$(CARGO) build --release

run-dev:
	@$(CARGO) run

run:
	@$(CARGO) run --release

install:
	@install -v -b -S .bak -m $(MODE) $(BIN_RELEASE) $(PREFIX)/bin

doc:
	@rm -rf $(DOC_DIR)
	@$(CARGO) doc -j 2 -v --offline --target-dir $(DOC_DIR) --open

test:
	@$(CARGO) test -- --test-threads=2