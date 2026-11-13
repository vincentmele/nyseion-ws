# rimu-ws on rimu-json, rimu-gzip and rimu-http1 — Phase 1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace rimu-ws's in-tree JSON writer, gzip encoder and HTTP/1.1 parser and body decoder with the rimu-json, rimu-gzip and rimu-http1 crates, with no HttpArena regression.

**Architecture:** Two small crate changes first (a buffer trait in rimu-json, a `view` method in rimu-http1), then three independent swaps in rimu-ws, each deleting the in-tree module. Then the HttpArena entries are updated, validated and A/B-benchmarked against the branch point.

**Tech Stack:** Rust (rimu-ws on its pinned nightly-2026-10-12; the crates on stable), path dependencies to `~/src/rimu-{json,gzip,http1}`, HttpArena (`~/src/HttpArena`: `scripts/validate.sh`, gcannon, wrk, zrk).

**Spec:** `docs/superpowers/specs/2026-11-13-rimu-crates-integration-design.md` (§1 constraints, §3 phase 1).

## Global Constraints

- Worktree: `~/src/rimu-ws/.claude/worktrees/rimu-crates`, branch `feat/rimu-crates` from `main` (record the SHA as **A**). Never work in `~/src/rimu-ws` itself.
- Crate changes are made in the crate's own repository (`~/src/rimu-json`, `~/src/rimu-http1`), written without wtx code; those crates stay Apache-2.0.
- Dependencies are path dependencies: `rimu-json = { path = "../../../rimu-json/rimu-json", optional = true }` from the worktree's `wtx/Cargo.toml`. Adjust the `../` depth to the worktree location; from the main checkout it is `../../rimu-json/rimu-json`. Same for `rimu-gzip/rimu-gzip` and `rimu-http1`.
- Feature names stay: `json-writer`, `gzip-encoder`, `http1-server-framework`.
- Zero allocation after warm-up: `wtx-internal/src/bin/arena_h1/alloc_tests.rs` and `arena_h2/alloc_tests_h3.rs` keep passing with unchanged assertions.
- Validator: HttpArena `scripts/validate.sh rimu-ws-h1` gives 55 passed, 0 failed, fragmentation 9 passed. `rimu-ws` (h2 entry) also passes its validator.
- No regression: B (the branch) vs A within A's own pass spread on every profile, by the method of `docs/benchmarks/2026-11-12-conformance-fixes-colossus/README.md`.
- Shared machine: `cargo -j 6` at most, one heavy job at a time, ≥15 GB available, temp files under `/mnt/nvme1tb/claude-tmp/`. Benchmarks only when no outside process uses more than half a CPU (the method's quiet check).
- Commits: author Vincent Mele, ending with the `Co-Authored-By` line. **No push** without the user's word.

## Review Focus

- **Pipelined requests across buffer compaction:** `Http1Conn` shifts `buf` when it compacts. The resumable parser's offsets are relative to `buf[start..]`, so after a compaction mid-head the parser must be reset or rebased. Expect the same answers as today; pinned by the test `head_split_across_a_compaction` in Task 5.
- **A view held while the next request arrives:** `request_survives_feed_after_ready` and `moving_the_conn_keeps_the_view_valid` (existing in `conn.rs`) must pass unmodified in what they assert.
- **Content-Length above `usize::MAX` on 32-bit:** the `u64` from rimu-http1 is checked into `usize`; overflow gets 413 as today's oversized-length rule. Pinned by the test `content_length_over_usize_is_rejected` in Task 5.
- **The JSON writer growing a `VectorUsize` from empty:** it must grow through the trait and keep the bytes. Pinned by `json_writer_grows_a_vector_from_empty` in Task 2.
- **gzip output of the json-comp bodies must not grow** past 958 / 1,313 / 1,544 bytes. Pinned by `json_comp_bodies_do_not_grow` in Task 3.

---

### Task 0: Worktree and baseline

**Files:** none changed.

- [ ] **Step 1: Create the worktree** with superpowers:using-git-worktrees at `~/src/rimu-ws/.claude/worktrees/rimu-crates`, branch `feat/rimu-crates` from `main`; record `git rev-parse HEAD` as A in `/mnt/nvme1tb/claude-tmp/rimu-crates/A.txt`.
- [ ] **Step 2: Baseline tests**: run `.scripts/internal-tests4.sh` and `cargo test -p rimu-ws --features http1-server-framework,http2-server-framework,io-uring,crypto-ring,json-writer,gzip-encoder,serde_json --lib`, and the `arena-h1` and `arena-h2` binary tests (`cargo test -p wtx-internal --features arena-h1 --bin arena-h1`, same for `arena-h2`). Expected: all pass. Save the output to `/mnt/nvme1tb/claude-tmp/rimu-crates/baseline-tests.log`.
- [ ] **Step 3: Baseline binaries**: build the HttpArena `rimu-ws-h1` and `rimu-ws` entries from A natively, as the conformance write-up did. Fill the entry with `local-src.sh <worktree>`, then `cargo build --release --locked` with `RUSTFLAGS="-C target-cpu=native"`. Copy the binaries to `/mnt/nvme1tb/claude-tmp/rimu-crates/A/`.

### Task 1: rimu-json — `JsonBuffer` trait (in `~/src/rimu-json`)

**Files:**
- Modify: `rimu-json/src/writer.rs` (constructor, `grow`, `Drop`), `rimu-json/src/lib.rs` (export)
- Test: `rimu-json/tests/buffer.rs`

**Interfaces:**
- Produces:
  ```rust
  /// # Safety
  /// `as_mut_ptr` points to `capacity()` bytes of which the first `len()` are initialized; after
  /// `reserve(n)`, `capacity() - len() >= n`; `set_len(n)` with `n <= capacity()` makes `n` the length.
  pub unsafe trait JsonBuffer {
    fn as_mut_ptr(&mut self) -> *mut u8;
    fn len(&self) -> usize;
    fn capacity(&self) -> usize;
    unsafe fn set_len(&mut self, len: usize);
    fn reserve(&mut self, additional: usize);
  }
  unsafe impl JsonBuffer for Vec<u8> { /* forwards to Vec */ }
  impl<'buf> JsonWriter<'buf> { pub fn new<B: JsonBuffer>(buf: &'buf mut B) -> Self; }
  ```
  `JsonWriter<'buf>` stays non-generic: it stores `buf: *mut ()` plus `grow_fn: unsafe fn(*mut (), len: usize, additional: usize) -> (*mut u8, usize)` and `set_len_fn: unsafe fn(*mut (), usize)`, monomorphized for `B` in `new`. `PhantomData<&'buf mut ()>` keeps the borrow. `with_sink` keeps working; its buffer is a `Vec<u8>`.

- [ ] **Step 1: Write the failing test** `tests/buffer.rs`. Define a test-only `struct RawBuf { ptr: *mut u8, len: usize, cap: usize }` backed by `std::alloc`, with `unsafe impl JsonBuffer`. Tests:
  - `writer_over_a_custom_buffer_matches_vec`: for the Item/Rating values from `tests/serde_parity.rs`, `JsonWriter::new(&mut RawBuf::new())` produces bytes equal to `rimu_json::to_vec(&value)`.
  - `custom_buffer_grows_from_zero_capacity`: 100,000 `w.str("x\"y")` into a zero-capacity `RawBuf` gives length 100,000 × 6 and the right bytes.
- [ ] **Step 2: Run** `cargo test -p rimu-json --test buffer`. Expected: FAIL (no `JsonBuffer`).
- [ ] **Step 3: Implement** the trait, the `Vec<u8>` impl and the erased constructor in `writer.rs`; export `JsonBuffer` from `lib.rs`.
- [ ] **Step 4: Run** `cargo test -p rimu-json --release --all-features` and `RIMU_JSON_NO_AVX2=1 cargo test -p rimu-json --release --all-features`, `cargo clippy --all-targets --all-features -- -D warnings`. Expected: all pass, clean.
- [ ] **Step 5: No regression**: `taskset -c 5 ./target/release/bench /mnt/nvme1tb/claude-tmp/codec-bench/corpus/api.json /mnt/nvme1tb/claude-tmp/codec-bench/corpus/license.txt` before and after. Derive rows within 5 %.
- [ ] **Step 6: Commit** in `~/src/rimu-json`: `JsonWriter over any buffer: the JsonBuffer trait`.

### Task 2: rimu-ws — JSON writer from rimu-json

**Files:**
- Modify: `wtx/Cargo.toml` (`json-writer = ["dep:rimu-json"]`, drop `zmij` if nothing else uses it), `wtx/src/misc.rs:54` (remove `pub use json_writer::JsonWriter`), `wtx/src/collections/vector.rs` (impl), `wtx-internal/src/bin/arena-h1.rs:282-320`, `wtx-internal/src/bin/arena-h2.rs:711-750`
- Delete: `wtx/src/misc/json_writer.rs`
- Test: `wtx/src/collections/vector.rs` tests module

**Interfaces:**
- Consumes: `rimu_json::{JsonBuffer, JsonWriter}` (Task 1).
- Produces: `unsafe impl<L: LinearStorageLen> rimu_json::JsonBuffer for Vector<L, u8>` behind `#[cfg(feature = "json-writer")]`; `rimu_ws::misc::JsonWriter` is gone. The arena binaries `use rimu_json::JsonWriter`; wtx-internal gets `rimu-json` as a direct optional dependency enabled by `arena-h1`/`arena-h2`.

- [ ] **Step 1: Write the failing test** `json_writer_grows_a_vector_from_empty` in `vector.rs` tests. Write 10,000 objects `{"k":"v\"","n":-12}` with `JsonWriter::new(&mut VectorUsize::<u8>::new())`; assert the bytes equal `serde_json::to_vec` of the same values and the length is unchanged after the writer drops.
- [ ] **Step 2: Run** `cargo test -p rimu-ws --features json-writer,serde_json --lib json_writer_grows`. Expected: FAIL (trait not implemented).
- [ ] **Step 3: Implement** the impl (`reserve` maps to `Vector::reserve`; on its error, abort with `handle_alloc_error`, the same failure `Vec` has). Delete `json_writer.rs` and the re-export. Port both `write_json` functions: `w.raw(b"…")` → `w.raw("…")`, drop the `?` on writer calls, keep each function's `rimu_ws::Result<()>` return for its other `?`s.
- [ ] **Step 4: Run** the Task 0 Step 2 commands, then the arena-h1 and arena-h2 tests (their JSON byte-equality and allocation tests). Expected: all pass.
- [ ] **Step 5: Commit** `json-writer: rimu-json instead of the in-tree writer`.

### Task 3: rimu-ws — gzip from rimu-gzip

**Files:**
- Modify: `wtx/Cargo.toml` (`gzip-encoder = ["dep:rimu-gzip"]`, `_fuzz-gzip` removed), `wtx/src/codec.rs:48-50`, `wtx-internal/src/bin/arena-h1.rs:417-470` (the `encode` function and the `GZIP` thread-local), `.scripts/internal-tests4.sh:19-20` (the two `gzip-encoder` check lines stay; they now check the dependency wiring)
- Delete: `wtx/src/codec/gzip_encoder.rs`, `wtx/src/codec/gzip_encoder/`, `wtx-fuzz/gzip_encoder.rs` and its `[[bin]]` entry in `wtx-fuzz/Cargo.toml`
- Test: `wtx-internal/src/bin/arena-h1.rs` tests (`json_gzip_uses_the_new_encoder`, `reused_gzip_encoder_matches_a_new_one` near line 835)

**Interfaces:**
- Consumes: `rimu_gzip::{Encoder, Level}`; `Encoder::with_level(Level::DEFAULT)`, `Encoder::gzip(&mut self, input: &[u8], out: &mut Vec<u8>)`.
- Produces: the thread-local becomes `RefCell<Option<rimu_gzip::Encoder>>`; `encode(Encoding::Gzip, …)` keeps its signature.

- [ ] **Step 1: Write the failing test** `json_comp_bodies_do_not_grow` in arena-h1's tests. For the json-comp profile's three bodies, `/json/25?m=4`, `/json/40?m=8` and `/json/50?m=6` (built with `write_json` from the dataset, as `docs/benchmarks/2026-11-11-json-comp-sweep` did), assert `encode(Encoding::Gzip, …)` output lengths ≤ 958, 1,313, 1,544. Point the two existing tests at `rimu_gzip::Encoder` (fresh vs reused outputs equal; output decodes with `zlib_rs` to the input).
- [ ] **Step 2: Run** `cargo test -p wtx-internal --features arena-h1 --bin arena-h1 gzip`. Expected: FAIL to compile (`rimu_gzip` missing).
- [ ] **Step 3: Implement**: dependency, thread-local and `encode`. Delete the in-tree encoder, its re-exports and the fuzz target.
- [ ] **Step 4: Run** the arena-h1 tests, `alloc_tests.rs` (json-comp allocation-free) and `.scripts/internal-tests4.sh`. Expected: all pass; lengths within the bounds.
- [ ] **Step 5: Commit** `gzip-encoder: rimu-gzip instead of the in-tree encoder`.

### Task 4: rimu-http1 — `RequestParser::view` (in `~/src/rimu-http1`)

**Files:**
- Modify: `src/request.rs`
- Test: `tests/semantics.rs`

**Interfaces:**
- Produces: `impl<const N: usize> RequestParser<N> { pub fn view<'p, 'b: 'p>(&'p self, buf: &'b [u8]) -> Result<Request<'p>, Error> }`. It returns the completed head's view of `buf` with one UTF-8 check of the head (`head_str(buf, pos, true)`) and no scan. Before completion, or after an error, it returns `Err(Error::InvalidResume)`. It does not change state.

- [ ] **Step 1: Write the failing tests** in `tests/semantics.rs`:
  - `view_after_complete_equals_parse`: for each corpus request, the fields from `view(buf)` equal those from the completing `parse(buf)`, including after the buffer is copied into a new `Vec` (moved).
  - `view_before_complete_is_invalid_resume`: on a partial head, `view` is `Err(Error::InvalidResume)`.
  - `view_does_not_reset`: `view` twice, then a `parse` of the same buffer, still returns `Complete`.
- [ ] **Step 2: Run** `cargo test --test semantics view_`. Expected: FAIL (no method).
- [ ] **Step 3: Implement** `view` in `request.rs`.
- [ ] **Step 4: Run** `cargo test --release`, `cargo clippy --all-targets -- -D warnings`, Miri on `tests/semantics.rs` (`cargo +nightly miri test --test semantics view_`). Expected: pass, clean.
- [ ] **Step 5: Commit** in `~/src/rimu-http1`: `RequestParser::view: the completed head again without re-parsing`.

### Task 5: rimu-ws — HTTP/1.1 core on rimu-http1

**Files:**
- Modify: `wtx/Cargo.toml` (`http1-server-framework` adds `dep:rimu-http1`), `wtx/src/http1.rs` (`Http1Request`, `BodyKind`, re-exports), `wtx/src/http1/conn.rs` (parser per connection, `parse_head`, `set_view`, body phase), `wtx/src/http/http1_server_framework.rs:55-75` (`Http1Req::header`, `headers`), any handler using `headers()` as a slice (arena-h1, `conn_driver.rs` tests)
- Delete: `wtx/src/http1/parser.rs`, `wtx/src/http1/body.rs`
- Create: `docs/http1-breaking-changes.md`
- Test: `wtx/src/http1/conn.rs` tests

**Interfaces:**
- Consumes: `rimu_http1::{RequestParser, Request, Status, Error, Headers, BodyDecoder, BodyStatus, BodyKind as WireBody, Method as WireMethod, Version as WireVersion}`; `RequestParser::view` (Task 4).
- Produces:
  - `pub struct Http1Request<'b> { pub method: Method, pub version: Version, pub path: &'b str, pub query: &'b str, pub headers: rimu_http1::Headers<'b>, pub body_kind: BodyKind, pub keep_alive: bool }`. `BodyKind` keeps `ContentLength(usize)`.
  - `const fn method_of(m: WireMethod) -> Method`, `const fn version_of(v: WireVersion) -> Version`, `fn status_of(e: rimu_http1::Error) -> StatusCode` (`StatusCode::try_from(e.status())`) in `http1.rs`.
  - `Http1Req::headers(&self) -> rimu_http1::Headers<'a>`; `Http1Req::header(&self, name) -> Option<&'a str>` = `headers.find(name)`.
  - `Http1Conn` field `parser: Box<rimu_http1::RequestParser<64>>`, replacing `scratch: Vec<ParseScratch>`. It is allocated once with the conn's parts, recycled with them and `reset()` when a message ends or the buffer is compacted.

- [ ] **Step 1: Write the failing tests** in `conn.rs`:
  - `head_split_across_a_compaction`: feed a pipelined first request plus half a second head, let the conn compact (`take_output`), feed the rest. The second request is parsed with the same fields as when fed whole.
  - `content_length_over_usize_is_rejected`: `Content-Length: 18446744073709551615` on a 64-bit build gets the same status today's oversized-length test expects (413).
  - `split_at_every_offset_matches_whole`: for 6 requests (GET, POST with Content-Length, chunked POST, pipelined pair, HTTP/1.0 keep-alive, `Expect: 100-continue`), feeding split at every offset gives the same events and responses as feeding whole.
- [ ] **Step 2: Run** `cargo test -p rimu-ws --features http1-server-framework,crypto-ring --lib http1::conn`. Expected: new tests FAIL (or don't compile until the types change).
- [ ] **Step 3: Implement**: the types and mapping functions, the per-connection parser in `conn.rs`, `set_view` via `view`, and the body phase on `rimu_http1::BodyDecoder::feed` with a sink appending to the body `Vec`. Delete `parser.rs` and `body.rs`. Update `headers()` users. Write `docs/http1-breaking-changes.md`, covering the `headers` type, the CONNECT target rule and the reordered error statuses (design §3.3).
- [ ] **Step 4: Run** all `conn.rs` tests (the 29 existing assertions unchanged except where a status changes per design §3.3; note each change in the commit message), the framework and `conn_driver.rs` tests, arena-h1 tests and `alloc_tests.rs`, and `.scripts/internal-tests*.sh`. Expected: all pass.
- [ ] **Step 5: Commit** `http1: rimu-http1's resumable parser and body decoder`.

### Task 6: HttpArena entries, validator, A/B

**Files:**
- Modify: `~/src/HttpArena/frameworks/rimu-ws-h1/local-src.sh` and `frameworks/rimu-ws/local-src.sh` (also `rsync` `~/src/rimu-json`, `~/src/rimu-gzip` and `~/src/rimu-http1`, excluding `target`, into the entry directory beside `rimu-ws/`), both `Dockerfile`s (`COPY rimu-json rimu-gzip rimu-http1 ./`), both `Cargo.lock`s
- Create: `docs/benchmarks/2026-11-XX-rimu-crates-phase1/README.md` and `data/`

- [ ] **Step 1: Entries**: make the edits, run `local-src.sh <worktree>` for both entries and `docker build` each. Expected: both images build.
- [ ] **Step 2: Validator**: `scripts/validate.sh rimu-ws-h1` and `scripts/validate.sh rimu-ws` from a scratch HttpArena clone, as in the conformance write-up. Expected: h1 55 passed / 0 failed / fragmentation 9 passed; h2 no new failures vs A.
- [ ] **Step 3: B binaries**: build natively as in Task 0 Step 3 into `/mnt/nvme1tb/claude-tmp/rimu-crates/B/`.
- [ ] **Step 4: A/B, quiet machine only**: copy `docs/benchmarks/2026-11-12-conformance-fixes-colossus/data/{ab.sh,ab-tls.sh,ab-zrk.sh,summary*.py}`. Run baseline, limited-conn, json-comp, json, json-tls, 8gbit (paced) and latency-10k for h1, plus json-h2c for the h2 entry. Same method: CPUs 0-5,12-17 for the servers, 6-10,18-22 for the generators, 4 passes alternating A,B / B,A, best of 3.
- [ ] **Step 5: Verdict**: per profile, B vs A against A's pass spread. Any profile worse beyond the spread blocks the swap that touches it (json/json-h2c → Task 2; json-comp → Task 3; the rest → Task 5); revert that task's commit on the branch and report. Write the README with the tables, the validator results and the verdicts.
- [ ] **Step 6: Commit** `docs: phase 1 rimu crates — validator and A/B` (rimu-ws). In HttpArena, `rimu-ws-h1, rimu-ws: entries build against the rimu crates`.

### Task 7: Merge gate — dependency sourcing (user decision)

- [ ] **Step 1: Stop and ask the user** how the crates become fetchable (design §3.4): publish `rimu-json` (+ `rimu-json-derive`), `rimu-gzip` and `rimu-http1` 0.1.0 to crates.io, or push their repositories to GitHub for git dependencies. Do not merge `feat/rimu-crates` to `main` or push anything until they answer.
- [ ] **Step 2: After the answer**, switch the three path dependencies to the chosen form, rerun Task 0 Step 2's tests and Task 6 Step 1's builds, commit `deps: rimu crates from <crates.io|git>`, and merge with superpowers:finishing-a-development-branch.
