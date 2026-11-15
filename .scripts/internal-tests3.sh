#!/usr/bin/env bash

. "$(dirname "$0")/common.sh" --source-only

# WTX

$rt test-with-features nyseion-ws serde
$rt test-with-features nyseion-ws serde_json
$rt test-with-features nyseion-ws sha3
$rt test-with-features nyseion-ws simdutf8
$rt test-with-features nyseion-ws std
$rt test-with-features nyseion-ws tokio
$rt test-with-features nyseion-ws tracing
$rt test-with-features nyseion-ws tracing-subscriber
$rt test-with-features nyseion-ws tracing-tree
$rt test-with-features nyseion-ws uuid
$rt test-with-features nyseion-ws web-socket,crypto-ring
$rt test-with-features nyseion-ws nyseion-ws-macros
$rt test-with-features nyseion-ws x509

# WTX Macros

$rt test-generic wtx-macros

# WTX UI

$rt check-generic nyseion-ws-ui
$rt test-with-features nyseion-ws-ui embed-migrations
$rt test-with-features nyseion-ws-ui schema-manager
$rt test-with-features nyseion-ws-ui schema-manager-dev
$rt test-with-features nyseion-ws-ui http-client
$rt test-with-features nyseion-ws-ui web-socket
