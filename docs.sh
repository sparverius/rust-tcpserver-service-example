#!/bin/sh

docs_open() {
    cargo doc --open --no-deps --manifest-path "$1"
}

# for test-client documentation
# DOC_TEST_CLIENT=test-client/Cargo.toml
# docs_open $DOC_TEST_CLIENT

DOC_SERVICE=service/Cargo.toml
docs_open $DOC_SERVICE
