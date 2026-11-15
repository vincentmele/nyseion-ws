#!/usr/bin/env bash

set -euxo pipefail

MIRIFLAGS="-Zmiri-disable-isolation" cargo miri test --features http2,postgres,web-socket -p nyseion-ws

pushd wtx-macros-tests
cargo run