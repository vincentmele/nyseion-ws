# rimu-ws on the rimu crates — design, 13 Nov 2026

rimu-ws's own implementations of five components were extracted into standalone Apache-2.0 crates
(`~/src/rimu-json`, `~/src/rimu-gzip`, `~/src/rimu-http1`, `~/src/rimu-qpack`, `~/src/rimu-uring`),
each since extended and tested on its own. This design makes rimu-ws depend on them and delete the
in-tree copies, so there is one implementation of each, and does it without losing any HttpArena
result, conformance result or the zero-allocation property.

## 1. Constraints

- **No regression.** Every swap is A/B-benchmarked on the HttpArena profiles it touches, by the
  method of `docs/benchmarks/2026-11-12-conformance-fixes-colossus` (A = the branch point, B = the
  swap; alternating order, 4 passes, best of 3 runs per pass and build, medians, server cpu per
  request). A swap whose B is worse than A by more than A's own pass spread on any profile is not
  merged until fixed. HttpArena's validator must stay at 55 passed / 9 fragmentation checks passed.
- **Zero allocation after warm-up** stays: rimu-ws's allocation tests (`wtx-internal/src/bin/
  arena_h1/alloc_tests.rs`, `arena_h2/alloc_tests_h3.rs`) must pass unchanged in what they assert.
- **Licensing.** rimu-ws stays MPL-2.0 and depends on the Apache-2.0 crates (allowed). Nothing
  moves the other way: changes needed in a crate are made in that crate's repository, written
  without wtx code, so the crates stay Apache-2.0 (see `~/src/rimu-uring/docs/provenance.md` for
  the check used).
- **Toolchain.** rimu-ws builds on its pinned nightly; the crates build on stable and on that nightly.
- **Public API changes** to rimu-ws are allowed (its users are the HttpArena entries and the soak
  harness), recorded in `docs/http1-breaking-changes.md` as `docs/http2-breaking-changes.md` did.

## 2. Phases

| phase | crates | why this order |
|---|---|---|
| 1 | rimu-json, rimu-gzip, rimu-http1 | Small surfaces (3, 3 and 2 files), crates complete, each swap independent |
| 2 | rimu-qpack | Feature-complete crate, but its API is a redesign: the h3 connection code (10 files) is rewired |
| 3 | rimu-uring | Foundation of all of rimu-ws (23 files); the crate lacks kernel TLS, first-packet forwarding and zero-copy UDP sends, which rimu-ws uses |

Each phase is a branch off `main` in a worktree under `.claude/worktrees/`, merged on its own.
Phase 1 has a full plan (`docs/superpowers/plans/2026-11-13-rimu-crates-phase1.md`); phases 2 and
3 get theirs when phase 1 is merged, from the outlines in §4 and §5.

## 3. Phase 1

### 3.1 rimu-json

- **Gap:** rimu-ws's JSON bodies are `VectorUsize<u8>` (its own raw storage, not a `Vec`): the h2
  handler writes into `state.req.msg_data.body`, the h1 handler into a thread-local `VectorUsize`.
  `rimu_json::JsonWriter::new` takes only `&mut Vec<u8>`.
- **Change in rimu-json:** an `unsafe trait JsonBuffer` (pointer, length, capacity, `set_len`,
  `reserve`) implemented for `Vec<u8>`; `JsonWriter::new<B: JsonBuffer>(&mut B)`. The writer stays
  non-generic, so `JsonWrite::write_json(&self, &mut JsonWriter<'_>)` is unchanged: it keeps the
  pointer, length and capacity it already caches, and reaches the buffer only on growth and drop,
  through two monomorphized function pointers stored at construction (the cold paths).
- **Change in rimu-ws:** `json-writer = ["dep:rimu-json"]`; `impl JsonBuffer for Vector<L, u8>`
  in `wtx/src/collections/vector.rs` (local type, so the orphan rule allows it); delete
  `wtx/src/misc/json_writer.rs`; the two `write_json` functions use the crate's API (`raw` takes
  `&str`, methods return nothing).

### 3.2 rimu-gzip

- `gzip-encoder = ["dep:rimu-gzip"]`; arena-h1's `encode` uses `rimu_gzip::Encoder` at
  `Level::DEFAULT` (level 3, the old tuned parameters) with `gzip(input, out)` into the response's
  `Vec<u8>`. Delete `wtx/src/codec/gzip_encoder*` and `wtx-fuzz/gzip_encoder.rs` (rimu-gzip has its
  own fuzz target). The json-comp sizes must not grow: 958 / 1,313 / 1,544 bytes for the 25 / 40 /
  50-item bodies (`docs/benchmarks/2026-11-11-json-comp-sweep`).

### 3.3 rimu-http1

- **Design change in rimu-ws:** each `Http1Conn` keeps one heap-allocated `RequestParser<64>`
  (allocated once per connection slot, like today's `ParseScratch`, so views survive moves of the
  conn) and calls it after every read; the parser resumes instead of re-scanning. The second parse
  in `set_view` becomes `RequestParser::view(buf)`, a new rimu-http1 method that rebuilds the view
  of the completed head with one UTF-8 check and no scan.
- **Public API change:** `Http1Request.headers` becomes `rimu_http1::Headers<'b>` (zero-copy view;
  `iter()`, `find()`), `Http1Req::headers()` returns it, `Http1Req::header(name)` uses `find`. Method
  and version map to rimu-ws's `Method`/`Version` in one `const fn` each; `Error::status()` maps
  to `StatusCode`; `BodyKind` gains rimu-http1's `u64` length (checked into `usize`).
- Delete `wtx/src/http1/parser.rs` and `body.rs`; `conn.rs` uses `rimu_http1::BodyDecoder::feed`.
- **Behaviour changes, accepted:** CONNECT targets must be `host:port` (34 generated inputs differ
  from today); 32 rejections get a different status because rimu-http1 judges bytes in order
  instead of checking UTF-8, size and bare LF over the whole head first. The validator decides.

### 3.4 Dependency sourcing

Path dependencies to the sibling checkouts (`wtx/Cargo.toml`: `path = "../../rimu-json/rimu-json"`)
work locally and in the HttpArena entries once `local-src.sh` copies the crates next to `rimu-ws/`.
They break rimu-ws's GitHub CI and any clone. **Before phase 1 merges to `main`, the crates must be
fetchable**: published to crates.io (recommended: `rimu-json` 0.1.0, `rimu-json-derive`, `rimu-gzip`,
`rimu-http1`) or pushed to GitHub and used as git dependencies. That is the user's call; the plan
stops at that gate.

## 4. Phase 2 outline — rimu-qpack

Replace `wtx/src/http3/qpack/` and `qpack_check.rs` with `rimu-qpack`: request decoding through
`Decoder::decode` with its sink, the encoder stream fed with `feed_encoder_stream`, decoder-stream
output drained into the control-stream writer, blocked streams parked and resumed from
`take_unblocked`; responses through `encode_static_prefix`/`encode_static_line` (pre-encoded heads)
or an `Encoder` with `InsertPolicy::Never`, as today. Verify with h3spec (86 passing today),
the curl/quic-go/ngtcp2 interop under loss (`.scripts/h3-interop.sh`), the h3 allocation tests and
baseline-h3/static-h3 A/B (`.scripts/h3-bench.sh`).

## 5. Phase 3 outline — rimu-uring

1. **In rimu-uring first** (Apache-2.0, written from the Linux kTLS and io_uring documentation, not
   from rimu-ws's TLS module): a kernel-TLS feature (`setsockopt(SOL_TLS)` with a caller-supplied
   cipher state, record send/receive with control messages, alert handling); cross-runtime UDP
   forwarding (`IORING_OP_MSG_RING`); zero-copy UDP sends (`SENDMSG_ZC`). Each with tests and
   benchmarks in the crate.
2. **In rimu-ws:** an adapter module implementing rimu-ws's own stream traits (`StreamReader`,
   `StreamWriter`, `TcpStream`/`TcpListener`, `Executor`) for rimu-uring's types — MPL code in the
   MPL repo; then switch the frameworks one at a time (WebSocket, h1, h2, h3), deleting
   `executor/uring_executor*` last. Every HttpArena profile and the soak suite re-verified.
3. Decision point before step 1: whether rimu-ws should run on rimu-uring at all, or keep its
   executor and let rimu-uring evolve separately.
