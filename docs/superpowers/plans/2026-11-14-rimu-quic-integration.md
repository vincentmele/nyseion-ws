# rimu-ws on rimu-quic, rimu-h3 and rimu-qpack — Implementation Plan

**Goal:** rimu-ws's HTTP/3 server framework on the crates; the in-tree QUIC/HTTP/3/QPACK stack
deleted; every result kept (validator, h3spec, interop, rimu-soak h3, allocation tests, A/B).

**Spec:** `docs/superpowers/specs/2026-11-14-rimu-quic-integration-design.md` (rulings R1–R6,
mapping §2, gaps §5, verification §6).

**Ledger:** `.superpowers/sdd/2026-11-14-rimu-quic-integration/progress.md` (rulings, results).

## Global constraints

- Worktree `~/src/rimu-ws/.claude/worktrees/h3-crates`, branch `feat/rimu-quic` from `main`
  (A = 2081c741). Crate symlinks beside it (R2). Never `~/src/rimu-ws` itself or other worktrees.
- Before every cargo build/test/fuzz and benchmark: `free -g`, "available" ≥ 17 GB, else wait.
  `cargo -j 4`, one heavy job at a time; target and temp under `/mnt/nvme1tb/claude-tmp/h3-crates`.
- No `cargo fmt` in rimu-ws; `git status` before every commit, unrelated formatting reverted.
- Tests one per process: `/mnt/nvme1tb/claude-tmp/h3-crates/run-each.sh <tree> <label> <args>`.
- Commits: author Vincent Mele <vince@mele.law>, message ends with the `Co-Authored-By` line. No
  push, merge or remotes.

## Task 0 — Worktree, symlinks, baseline (A)

- Worktree and symlinks; baseline of A in an exported tree (`/mnt/nvme1tb/claude-tmp/h3-crates/
  base`, `git archive 2081c741`): lib (feature set of spec §6.1) and arena-h2, one test per process.

## Task 1 — rimu-quic: gap G1 (`~/src/rimu-quic`)

- Test first (`rimu-quic/tests/tx_arena_public.rs`): from outside the crate, write two datagrams
  with `begin`/`commit`, see one train in `pending`, `mark_submitted`/`release` it, and
  `max_trains() >= 64`. RED: does not compile (private methods).
- Make `TxArena::{begin, commit, max_trains}` public, documented. GREEN; `cargo test -p rimu-quic`
  passes. Commit in `~/src/rimu-quic`; `git pull --ff-only` in the clean clone.

## Task 2 — rimu-ws: dependencies and features

- `wtx/Cargo.toml`: optional path deps `rimu-quic` (no default features), `rimu-h3`, `rimu-qpack`.
  `quic = ["dep:rimu-quic", "rimu-quic/rustls", "std", ...]`, `crypto-ring` → `rimu-quic?/ring`,
  `crypto-alr` → `rimu-quic?/aws-lc-rs`, `http3 = ["http", "quic", "dep:rimu-h3", "dep:rimu-qpack"]`,
  `_h3-loopback-client` → `rimu-quic/client`.

## Task 3 — rimu-ws on the crates (the swap)

All at once, as one type flows through all of them (`TxArena` from `udp.rs` into the endpoint):
- `executor/uring_executor/udp.rs`: `crate::quic::*` → `rimu_quic::*`.
- `http3_server_framework.rs` and `endpoint_task.rs`: `rimu_quic::Endpoint<H3Server<...>>`,
  `RustlsConfig`, the `QuicRng` adapter, stats from the crates' counters, no TLS pool hooks.
- `router_handler.rs`: `impl rimu_h3::Handler for RouterHandler`; `ResponseHead`/`DateLine` on
  `rimu_qpack::encode_static_*` and `Vec<u8>`; `SharedBytes` → `rimu_quic::SharedBytes` (R5);
  errors (R6).
- `http3/loopback_client.rs` rewritten on `rimu_quic::client::TestClient` + rimu-qpack (same API:
  `H3LoopbackClient::{connect, close, poll, send, take, get, pipeline, send_pending_only}`,
  `H3Get`, `H3Reply`).
- Framework tests (`tests.rs`) adapted (TLS config, pool sizes in bytes); arena-h2 and
  soak-server-h3 build their `RustlsConfig` from the PEM pair; the churn test's allowance (§3 of
  the spec) measured and documented.
- Verify: lib framework tests and arena-h2, one per process, against the baseline.

## Task 4 — Delete the in-tree stack

- `wtx/src/quic/`, `wtx/src/http3/` (but `http3.rs` re-exports + `loopback_client.rs`),
  `tls/quic_tls.rs` and its hooks, `http/prefix_int.rs`, QUIC-only crypto (`crypto/aead_key*`,
  `crypto/hp_key*`), wtx-fuzz targets, `_fuzz-conn`; `Error` variants; `lib.rs` re-exports
  (`pub use rimu_quic as quic`). `.scripts/fuzz-h3.sh` points at the crates.
- Verify: `cargo check` of the feature sets of `.scripts/internal-tests4.sh` that involve quic/h3;
  full lib + arena-h2 suites against the baseline. Count the deleted lines.

## Task 5 — `docs/http3-breaking-changes.md`

## Task 6 — HttpArena entry

- Worktree `~/src/HttpArena/.claude/worktrees/rimu-quic` from branch `rimu-crates` (has phase 1's
  `local-src.sh` and entry layout; ledger says which). `local-src.sh` copies `rimu-quic/` (the
  workspace: `rimu-quic/`, `rimu-h3/`, root `Cargo.toml`) and `rimu-qpack/` beside `rimu-ws/`; the
  Dockerfile COPYs them. Validator `scripts/validate.sh rimu-ws`: 32/0 + WS 7; h3 checks if the
  local `httparena-h2load-h3` image serves.

## Task 7 — Conformance and interop

- Allocation tests (Docker if needed), h3spec, interop matrix, rimu-soak h3 — servers in Docker
  (`--network host --ulimit memlock=-1:-1 --ulimit nofile=1048576:1048576 --security-opt
  seccomp=unconfined`), ports checked free first, containers stopped by name.

## Task 8 — Performance A/B

- A (main) and B (branch) arena-h2 images built the same way; baseline-h3 and static-h3 with
  the `h3-bench.sh` shapes, identical cpusets, alternating, best of 3 per start, `uptime`
  recorded; medians, spreads, server CPU per request.
