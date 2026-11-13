# Breaking changes — rimu-json, rimu-gzip and rimu-http1 (13 Nov 2026)

rimu-ws now uses three standalone crates instead of its in-tree copies: rimu-json for the JSON
writer, rimu-gzip for the gzip encoder and rimu-http1 for HTTP/1.1 parsing. The sections below list
what changes for users of each.

## JSON writer (rimu-json)

- `rimu_ws::misc::JsonWriter` is a re-export of `rimu_json::JsonWriter`.
- `raw` takes `&str` (was `&[u8]`), so the output stays UTF-8; `end_seq` takes an ASCII byte.
- The methods return nothing (they returned `crate::Result<()>`): drop the `?` after each call.
- `JsonWriter::new` takes any `B: rimu_json::JsonBuffer`; `Vec<u8>` and rimu-ws's `VectorUsize<u8>`
  implement it.

## gzip (rimu-gzip)

- `rimu_ws::codec::{GzipEncoder, GzipLevel, GzipParams, GzipStrategy}` re-export
  `rimu_gzip::{Encoder, Level, Params, Strategy}`.
- `GzipEncoder::encode(input, out) -> Result` is now `gzip(input, out)`, which returns nothing
  (`deflate` and `zlib` write the other two formats). `GzipEncoder::new()` is the default level,
  the parameters the in-tree encoder used, and writes the same sizes for the json-comp bodies.
- `GzipParams` has `strategy` (`LazyOnce` is the old `lazy: true`, `Greedy` the old `false`) and
  `split_blocks` (the old encoder never split) in place of `lazy`.
- Removed: `codec::gzip_check` and the `_fuzz-gzip` feature (rimu-gzip carries its own fuzz target).

## HTTP/1.1 (rimu-http1)

rimu-ws's HTTP/1.1 core now parses requests and decodes bodies with
[rimu-http1](https://github.com/vincentmele/rimu-http1) instead of its own `http1/parser.rs` and
`http1/body.rs`. Each connection keeps one resumable parser: a head that arrives over several reads
is scanned once, not again on every read. What changes for users of the framework:

### API

- `Http1Request::headers` is a `rimu_ws::http1::Headers<'b>` (rimu-http1's zero-copy view of the
  fields), no longer a `&[(&str, &str)]`. Use `headers.find(name)` (ignores case), `headers.get(i)`,
  `headers.len()` and `headers.iter()`, which yields `Header { name, value }`.
- `Http1Req::headers()` returns that `Headers<'a>`; `Http1Req::header(name)` is unchanged.
- Removed: `http1::{parse_request, ParseOutcome, ParseScratch, error_status, BodyDecoder,
  BodyStatus}`. `Http1Conn` and `Http1ConnParts` keep their API; the parts hold the parser where they
  held the parse scratch.

### Behaviour

- **CONNECT** targets must be authority-form with a port (`host:port`), and other methods may not
  use authority-form (RFC 9112 §3.2.3); such requests were accepted before and are now answered 400.
- **Error statuses** follow the order in which bytes arrive: an error is reported as soon as the
  bytes that cause it are read. Before, the head's size, UTF-8 and bare-LF checks ran over the whole
  head first, so a few malformed heads that break two rules get the status of the earlier rule now
  (for example 400 instead of 431).
- An absolute-form target still replaces `Host` (RFC 9112 §3.2.3), as before.
- Limits are as before: a head of at most 16 KiB and 64 fields, with no shorter limit per line
  (rimu-http1's own default of 8 KiB per line is raised to 16 KiB).
- On 32-bit targets, a `Content-Length` larger than `usize` is answered 413, like any body over the
  limit (on 64-bit every valid length fits, and lengths over the limit get 413 as before).
