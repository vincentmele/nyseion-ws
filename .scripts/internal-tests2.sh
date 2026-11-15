#!/usr/bin/env bash

. "$(dirname "$0")/common.sh" --source-only

# WTX

$rt test-with-features nyseion-ws grpc-server,crypto-ring
$rt test-with-features nyseion-ws hashbrown
$rt test-with-features nyseion-ws http
$rt test-with-features nyseion-ws http2-client-pool,crypto-ring
$rt test-with-features nyseion-ws http-cookie
$rt test-with-features nyseion-ws http-cookie-secure
$rt test-with-features nyseion-ws http-session,crypto-ring
$rt test-with-features nyseion-ws http2,crypto-ring
$rt test-with-features nyseion-ws http2-server-framework,crypto-ring
$rt test-with-features nyseion-ws http3,web-socket,crypto-ring
$rt test-with-features nyseion-ws http3-server-framework,web-socket,crypto-ring
$rt test-with-features nyseion-ws httparse
$rt test-with-features nyseion-ws libc
$rt test-with-features nyseion-ws macros
$rt test-with-features nyseion-ws memchr
$rt test-with-features nyseion-ws optimizations
$rt test-with-features nyseion-ws parking_lot
$rt test-with-features nyseion-ws portable-atomic
$rt test-with-features nyseion-ws portable-atomic-util
$rt test-with-features nyseion-ws postgres
$rt test-with-features nyseion-ws quick-protobuf
$rt test-with-features nyseion-ws rand_core
$rt test-with-features nyseion-ws rust_decimal
$rt test-with-features nyseion-ws schema-manager
$rt test-with-features nyseion-ws schema-manager-dev
$rt test-with-features nyseion-ws secret,crypto-ring