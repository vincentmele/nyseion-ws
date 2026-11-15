#!/usr/bin/env bash

. "$(dirname "$0")/common.sh" --source-only

# WTX

$rt check-generic nyseion-ws
$rt test-with-features nyseion-ws _bench
$rt test-with-features nyseion-ws _hack
$rt test-with-features nyseion-ws _integration-tests
$rt test-with-features nyseion-ws _tracing-tree
$rt test-with-features nyseion-ws 32-tuple-impls
$rt test-with-features nyseion-ws arbitrary
$rt test-with-features nyseion-ws argon2
$rt test-with-features nyseion-ws asn1
$rt test-with-features nyseion-ws client-api-framework
$rt test-with-features nyseion-ws crossbeam-channel
$rt test-with-features nyseion-ws crypto
$rt test-with-features nyseion-ws crypto-alr,_hack
$rt test-with-features nyseion-ws crypto-graviola
$rt test-with-features nyseion-ws crypto-ring
$rt test-with-features nyseion-ws database
$rt test-with-features nyseion-ws database-tests
$rt test-with-features nyseion-ws default
$rt test-with-features nyseion-ws embassy-net,_hack
$rt test-with-features nyseion-ws embassy-time,_hack
$rt test-with-features nyseion-ws executor
$rt test-with-features nyseion-ws foldhash
$rt test-with-features nyseion-ws getrandom
$rt test-with-features nyseion-ws grpc
$rt test-with-features nyseion-ws grpc-client,crypto-ring