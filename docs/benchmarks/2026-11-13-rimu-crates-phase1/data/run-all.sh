#!/usr/bin/env bash
# Phase 1 A/B, one profile family after the other: gcannon (baseline, limited-conn, json-comp),
# wrk (json-tls, 8gbit open loop), zrk (8gbit paced, latency-10k), h2load (json-h2c).
set -u
here=$(cd "$(dirname "$0")" && pwd)
L=/mnt/nvme1tb/claude-tmp/rimu-crates/entries
for s in ab ab-tls ab-zrk ab-h2c; do
  echo "#### $s $(date -u +%T)"
  bash "$here/$s.sh" > "$L/$s.log" 2>&1 || echo "$s exited $?"
done
echo "#### all done $(date -u +%T)"
