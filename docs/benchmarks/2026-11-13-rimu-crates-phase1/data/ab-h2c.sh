#!/usr/bin/env bash
# ab-h2c.sh: json-h2c A/B of the rimu-ws (h2) entry, A = main 2081c741, B = feat/rimu-crates
# (rimu-json in the JSON handler). Each run is .scripts/h2-bench.sh (server on 0-5,12-17, h2load on
# 6-11,18-23, 1,024 connections, best of 3 runs of 5 s per start), 4 passes in the order A,B then
# B,A, each pass after the machine is quiet.
set -u
R=/mnt/nvme1tb/claude-tmp/rimu-crates/entries; HA=${HA:-$HOME/src/HttpArena}
REPO=$(cd "$(dirname "$0")/../../../.." && pwd)
O=${O:-$R/out-h2c}; mkdir -p $O; PASSES=${PASSES:-4}
quiet() { # until no process outside the run uses > 50 % of a CPU (max 45 min)
  for _ in $(seq 1 270); do
    busy=$(ps -eo pcpu=,comm= --sort=-pcpu | awk '$1 > 50 && $2 !~ /^(rtk|bash|ps|awk|sleep|h2load|docker.*|containerd.*|httparena-rimu-w.*)$/ {print $2" "$1}' | head -3)
    [ -z "$busy" ] && return 0; echo "waiting: $(echo $busy | tr '\n' ' ') load $(cut -d' ' -f1-3 /proc/loadavg)"; sleep 10
  done; echo "machine never went quiet: $busy"; return 1
}
for pass in $(seq 1 $PASSES); do
  if [ $((pass % 2)) = 1 ]; then order="A B"; else order="B A"; fi
  quiet || exit 1
  echo "== pass $pass $(date -u +%T) order $order load $(cut -d' ' -f1-3 /proc/loadavg)"
  for v in $order; do
    line=$(ARENA=$HA ARENA_BIN=$R/$v/target-rimu-ws/release/httparena-rimu-ws TMPDIR=$O \
      bash "$REPO/.scripts/h2-bench.sh" rimu json-h2c 1024 2>$O/err-$v-p$pass.log)
    echo "p$pass $v $line" | tee -a $O/results.txt
  done
done
echo "== done $(date -u +%T) load $(cut -d' ' -f1-3 /proc/loadavg)"
