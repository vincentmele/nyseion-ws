# rimu-ws on rimu-quic, rimu-h3 and rimu-qpack — design, 14 Nov 2026

Step 6 of `~/src/rimu-quic/docs/design.md` (§8): rimu-ws's HTTP/3 server framework runs on the
standalone Apache-2.0 crates **rimu-quic** (QUIC transport, rustls adapter), **rimu-h3** (HTTP/3)
and **rimu-qpack** (QPACK), and the in-tree `quic/`, `http3/` and `tls/quic_tls.rs` are deleted,
without losing any result: the HttpArena validator, h3spec, interop, rimu-soak HTTP/3, the
allocation tests and performance.

Precedent: phase 1 (`.claude/worktrees/rimu-crates`, `docs/superpowers/specs/2026-11-13-rimu-crates-
integration-design.md`), whose method and rulings carry over (re-exports, path dependencies through
symlinks to clean clones, one test per process, no `cargo fmt`, the memlock limit).

## 1. Rulings taken before the work

- **R1 — rimu-ws keeps its own executor and HTTP/3 worker loop.** `http3_server_framework/
  endpoint_task.rs` stays and keeps driving the worker on `executor/uring_executor/udp.rs`
  (rimu-ws's io_uring UDP: GRO, GSO trains, zero-copy sends, inboxes between workers). Only the
  sans-I/O layers are swapped: `quic/` → `rimu_quic::{Endpoint, QuicApp, TxArena, ...}`,
  `http3/` → `rimu_h3::{H3Server, Handler, ...}`, `http3/qpack*` → rimu-qpack (through rimu-h3),
  `tls/quic_tls.rs` → rimu-quic's rustls adapter (`rimu_quic::rustls::RustlsConfig`). rimu-quic's
  own `uring` driver (on rimu-uring) is **not** used: moving rimu-ws onto rimu-uring is a separate,
  later phase. rimu-ws's router and static-file handling become a `rimu_h3::Handler`
  implementation in rimu-ws (`router_handler.rs`, MPL code in the MPL repository).
- **R2 — path dependencies through symlinks to clean clones.** From `wtx/Cargo.toml`:
  `../../rimu-quic/rimu-quic`, `../../rimu-quic/rimu-h3`, `../../rimu-qpack` (right for
  `~/src/rimu-ws` and for the HttpArena entry layout `entry/rimu-ws/wtx` → `entry/rimu-quic/...`).
  The worktree is `.claude/worktrees/h3-crates` (the name `rimu-quic` is taken by the symlink);
  `.claude/worktrees/rimu-quic` → `/mnt/nvme1tb/claude-tmp/rimu-crates/crates/rimu-quic` (clean
  clone at c1da98b, then at the commits of §5), `.claude/worktrees/rimu-qpack` → `.../crates/
  rimu-qpack` (e5bbbad). Never `~/src/rimu-*`, which other sessions edit. rimu-h3's own
  `../../rimu-qpack` resolves to the same symlink. rimu-quic's optional `rimu-uring` path
  dependency is never activated and Cargo does not need it on disk (checked: `cargo metadata`
  resolves without it).
- **R3 — rimu-quic changes** (if the swap needs any) are made in `~/src/rimu-quic` as their own
  commits with tests, then pulled into the clean clone; rimu-qpack is not changed unless essential.

## 2. Mapping: what the framework, arena-h2 and the soak server use

| in-tree (rimu-ws) | crate | notes |
|---|---|---|
| `quic::Endpoint<A, RNG, TCX>` | `rimu_quic::Endpoint<A>` | `new(params, worker, workers, tls, &mut rng, loads, keys)`: `tls` is `Arc<dyn ServerSessionFactory>` (an `Arc<RustlsConfig>`), `rng` any `rimu_quic::rng::Rng` |
| `recv`, `recv_forwarded`, `forward_dropped`, `forward_segment`, `on_timeout`, `next_deadline`, `flush`, `begin_shutdown`, `abort_all`, `trim_idle`, `stats`, `pool_bytes`, `with_conn` | same names, same semantics | `pool_bytes` was test-feature-only in-tree; public in the crate |
| `endpoint.pools().conn_cold.len()` (framework tests) | `endpoint.pool_bytes()` | the pool sizes of the idle-trim test are read as bytes |
| `quic::{EndpointKeys, WorkerLoads, RecvOutcome, FlushOutcome, DatagramMeta, QuicApp, ConnHandle, StreamId, BodyRef, QuicParams, QuicLimits, EndpointStats}` | same names in `rimu_quic` | `QuicParams`/`QuicLimits` have identical setters and fields (diffed) |
| `quic::{steering_cbpf, SteeringProgram, STEERING_PROGRAM_LEN}` (udp.rs, endpoint_task) | `rimu_quic::{...}` (Linux) | rimu-ws links rimu-quic, not rimu-uring |
| `quic::{TxArena, TrainId, TxTrain}` (udp.rs) | `rimu_quic::{...}` | udp.rs needs `TxArena::max_trains` (and its tests `begin`/`commit`), `pub(crate)` in the crate: **gap G1** |
| `tls::QuicTls` (wtx's TLS 1.3 handshake), its pooled states, `tls::{trim_idle_pools, pooled_state_bytes, pooled_states}` for QUIC | `rimu_quic::rustls::RustlsConfig` | rustls does the handshake; no pooled TLS state, so the pool-bytes and idle-trim accounting drop that term |
| `Http3ServerFramework::new(executor, rng, TlsConfig<TCX>)` | `new(executor, rng, RustlsConfig)` | **breaking**: wtx's `TlsConfig` cannot hand its private key to rustls; `RustlsConfig::builder()` takes the PEM/DER pair, ALPN `h3` by default |
| wtx `CryptoRng + CryptoSeedableRng` generators | `rimu_quic::rng::Rng` | a small adapter (`QuicRng`) in the framework; per-worker generators still derived from the process's as today |
| `http3::H3App<H>` | `rimu_h3::H3Server<H>` | `bytes()` → `pooled_bytes() + scratch_bytes()`; `qpack_counters`, `pooled`, `trim_idle`, `resume`, `handler` same |
| `http3::{H3Handler, H3ReqCtx, H3Request, ReqOutcome}` | `rimu_h3::{Handler, ReqCtx, Request, ReqOutcome}` | same shape (the crate is a port); handler methods return `rimu_h3::Result` |
| `http3::{encode_static_line, encode_static_prefix}` (router handler's `ResponseHead`, `DateLine`) | `rimu_qpack::{...}` on `Vec<u8>` | direct dependency on rimu-qpack (ruling R4) |
| `misc::SharedBytes` → `BodyRef::try_from` | `rimu_quic::SharedBytes` → `BodyRef` | rimu-ws keeps its `SharedBytes` (StaticFiles, h2); a zero-copy conversion moves its `Arc<[u8]>` into the crate's (ruling R5) |
| `http3::loopback_client` (in-tree client `QuicConn`, `QpackDecoder`) | rewritten on `rimu_quic::client::TestClient` + `rimu_qpack` | feature `_h3-loopback-client` (test-only) turns on rimu-quic's `client` |
| `http3::{H3Params, H3ErrorCode, Http3Error}` (public) | `rimu_h3::{...}` re-exported | `H3Params` gains `content_length`, `date`, `response_encoder`; `Http3Error`'s QPACK variants are `rimu_qpack::Error` now |
| `Error::{Quic(QuicError), QuicClose(..), Http3(Http3Error), Http3Close(..)}` | `Error::{Quic(rimu_quic::Error), Http3(rimu_h3::Error)}` | **breaking** |
| `Http3WorkerStats` (SIGUSR1 `h3-stats` of arena-h2) | unchanged | filled from `EndpointStats`, `QpackCounters`, the handler's counters and `UdpStats` as today; every counter has a source in the crates (no gap) |
| `quic` feature with `crypto-graviola`/`crypto-ruco` | `crypto-ring` (→ `rimu-quic/ring`) or `crypto-alr` (→ `rimu-quic/aws-lc-rs`) | **breaking**: the crates' packet keys are ring or aws-lc-rs |
| wtx-fuzz `quic_*`, `h3_conn`, `qpack_decoder`; `quic/{frame,packet}_check.rs`, `conn/check`, `endpoint/check`, `http3/{h3,qpack}_check.rs` | the crates' `fuzz/` and check modules | deleted here |
| `http/prefix_int.rs` (QPACK integers), the non-`http2` branch of `http/huffman.rs` | rimu-qpack | deleted; Huffman stays for HPACK |

Rulings from the mapping:
- **R4** — rimu-ws depends on rimu-qpack directly (beside rimu-h3, same clean clone) for the
  static-table encoding of response heads (`ResponseHead` of `router_handler.rs`, kept: it reads
  wtx `Headers` without building a slice of pairs, which `rimu_h3::ResponseHeads` would need per
  response) and for the test client. Cost if wrong: one more path dependency in the entry.
- **R5** — `impl From<misc::SharedBytes> for rimu_quic::SharedBytes` moves the `Arc<[u8]>` and
  the range (no copy, no atomic operation); with `portable-atomic-util` (another `Arc`) it copies.
- **R6** — errors of rimu-ws code inside `Handler` callbacks (a request buffer that cannot grow, a
  slot that cannot be made, a resume of an unknown slot) become `rimu_h3::Error::Http3(
  Http3Error::OutOfMemory)` (the stream is reset with `H3_INTERNAL_ERROR`, as before) and the
  error goes to the error callback first when it is not an allocation failure.

## 3. Behaviour changes

- **TLS for QUIC is rustls** (rimu-quic's adapter; spike `~/src/rimu-quic/docs/spike-tls.md`):
  +6–8 % CPU per ECDSA handshake; rustls allocates per handshake, which the pooled in-tree
  handshake did not (rimu-quic's README: 78 allocations with ring and ECDSA P-256, 87 with
  RSA-2048, the HttpArena certificate). The churn allocation test of arena-h2
  (`h3_connection_churn_allocates_only_crypto_budget`) gets a documented allowance: the per-
  handshake count measured on the arena's RSA-2048 certificate, named as rustls's share, in place
  of ring's RSA signature's 9. The steady-state tests keep asserting zero.
- **Resumption, 0-RTT and session tickets** stay off (rimu-quic's configuration); h3spec's 0-RTT
  case stays pending.
- **QPACK encoder-stream errors**: an encoder-stream instruction whose integer never ends is
  `QPACK_ENCODER_STREAM_ERROR` (0x0201) at once (rimu-qpack), where rimu-ws buffered it up to a cap
  and closed with `H3_EXCESSIVE_LOAD` (0x0107) (rimu-quic design §10). rimu-soak's
  `qpack_backlog` accepts either with `--qpack-backlog-codes`.
- Other error codes come from the crates' ports of the same code; any difference found by h3spec,
  interop or soak is recorded in the ledger.
- `H3Params` has three more fields. The framework's `set_content_length`/`set_date`/
  `set_rfc9110_headers` stay and set them; `response_encoder` is not used by the framework, whose
  router handler encodes heads with the static table (documented on the setter).

## 4. Deletions

`wtx/src/quic/**`, `wtx/src/http3/**` but a module of re-exports and the rewritten loopback client,
`wtx/src/tls/quic_tls.rs` and its hooks in `tls.rs`/`tls_pool.rs`, `wtx/src/http/prefix_int.rs`,
the QUIC header-protection keys of `crypto.rs`/`crypto/hp_key*` and `crypto/aead_key*` where only
QUIC used them, the wtx-fuzz targets of §2 and the features `_fuzz-conn`. The public modules
`rimu_ws::quic` (re-export of `rimu_quic`) and `rimu_ws::http3` (re-exports of `rimu_h3` plus the
test-only loopback client) stay so that paths like `rimu_ws::quic::QuicParams` keep working.
Every breaking change goes into `docs/http3-breaking-changes.md`.

## 5. Gaps in the crates

- **G1 (rimu-quic):** `TxArena::max_trains`, `begin` and `commit` are `pub(crate)`; an I/O layer
  outside the crate that keeps a view of each submitted train (rimu-ws's `udp.rs`) needs
  `max_trains`, and writing a datagram of one's own (udp.rs's tests, a non-QUIC protocol on the
  same socket) needs `begin`/`commit`. Made public with docs and a test of the public path.
- Found during the work: added to the ledger and here.

## 6. Verification

Each against the baseline of `main` (A = 2081c741), with the method of phase 1:

1. **Tests, one per process** (`/mnt/nvme1tb/claude-tmp/h3-crates/run-each.sh`, release): the
   rimu-ws lib with `http1-server-framework,http2-server-framework,http3-server-framework,io-uring,
   crypto-ring,web-socket,_h3-loopback-client` (baseline 1,055/1,060: five failures at A, all
   environment — four io_uring ENOMEM under the 8 MiB per-user memlock and one debug-assertion test
   in a release build), and arena-h2 (baseline 13/14: `h3_pool_memory_stays_within_the_budget`,
   GRO ring ENOMEM). The QUIC/HTTP/3 unit tests leave with the code (they live in the crates); the
   framework tests stay, adapted. Tests failing natively for rings are re-run in Docker
   (`--ulimit memlock=-1:-1 --security-opt seccomp=unconfined`).
2. **Allocation tests** (`arena_h2/alloc_tests_h3.rs`): zero per request after warm-up
   (baseline-h3 and static-h3 loads); handshake churn within the rustls allowance; pool memory
   within the budget (Docker if native rings do not suffice).
3. **HttpArena validator**: `scripts/validate.sh rimu-ws` 32 passed / 0 failed, WebSocket 7 passed;
   h3 checks too if the validator can use the local `httparena-h2load-h3` image.
4. **h3spec** (`.scripts/h3spec.sh`): 86 examples, 0 failures, 1 pending (0-RTT).
5. **Interop** (`.scripts/h3-interop.sh`; `rimu-h3-interop-curl`, `rimu-h3-interop-quic-go`,
   `udp-impair` from rimu-soak): the full matrix passes.
6. **rimu-soak h3 scenarios** (rimu-soak at its committed HEAD; `--ok-path`/`--ok-body` matching
   the arena routes).
7. **Performance A/B** in Docker with identical cpusets: A (main) vs B (this branch), baseline-h3
   and static-h3 with the `h3-bench.sh` shapes (h2load-h3 image), alternating, best of 3 per start,
   `uptime` recorded; medians, spreads, server CPU per request; no claim inside the spread.
