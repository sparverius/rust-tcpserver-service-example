#!/bin/sh

unit_test() {
    cargo test --release
}

client_test() {
    cargo -q run --release --bin test-client
}

show_help() {
    echo "./test.sh COMMAND"
    echo "COMMANDS"
    echo "      unit    run unit tests"
    echo "    client    run test-client"
}

case "$1" in
     unit) unit_test ;;
     client) client_test ;;
     *) show_help ;;
esac
