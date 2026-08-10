bin_release := "target/release/cocom"
doc_dir := "doc/"
prefix := "/usr/local"
mode := "711"

# List available recipes
default:
    @just --list

# Debug build
build-dev:
    cargo build

# Release build
build:
    cargo build --release

# Build and run (debug)
run-dev:
    cargo run

# Build and run (release)
run:
    cargo run --release

# Install the release binary to {{prefix}}/bin
install:
    install -v -m {{mode}} {{bin_release}} {{prefix}}/bin

# Generate documentation into {{doc_dir}}
doc:
    rm -rf {{doc_dir}}
    cargo doc -j 2 -v --offline --target-dir {{doc_dir}} --open

# Run the test suite
test:
    cargo test -- --test-threads=2
