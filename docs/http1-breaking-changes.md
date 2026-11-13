# HTTP/1.1 breaking changes — rimu-http1 (13 Nov 2026)

rimu-ws's HTTP/1.1 core now parses requests and decodes bodies with
[rimu-http1](https://github.com/vincentmele/rimu-http1) instead of its own `http1/parser.rs` and
`http1/body.rs`. Each connection keeps one resumable parser: a head that arrives over several reads
is scanned once, not again on every read. What changes for users of the framework:

## API

- `Http1Request::headers` is a `rimu_ws::http1::Headers<'b>` (rimu-http1's zero-copy view of the
  fields), no longer a `&[(&str, &str)]`. Use `headers.find(name)` (ignores case), `headers.get(i)`,
  `headers.len()` and `headers.iter()`, which yields `Header { name, value }`.
- `Http1Req::headers()` returns that `Headers<'a>`; `Http1Req::header(name)` is unchanged.
- Removed: `http1::{parse_request, ParseOutcome, ParseScratch, error_status, BodyDecoder,
  BodyStatus}`. `Http1Conn` and `Http1ConnParts` keep their API; the parts hold the parser where they
  held the parse scratch.

## Behaviour

- **CONNECT** targets must be authority-form with a port (`host:port`), and other methods may not
  use authority-form (RFC 9112 §3.2.3); such requests were accepted before and are now answered 400.
- **Error statuses** follow the order in which bytes arrive: an error is reported as soon as the
  bytes that cause it are read. Before, the head's size, UTF-8 and bare-LF checks ran over the whole
  head first, so a few malformed heads that break two rules get the status of the earlier rule now
  (for example 400 instead of 431).
- An absolute-form target still replaces `Host` (RFC 9112 §3.2.3), as before.
- A `Content-Length` larger than the platform's `usize` is answered 413, like any body over the
  limit.
