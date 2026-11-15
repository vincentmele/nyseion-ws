# HTTP/3 breaking changes (feat/nyseion-quic)

nyseion-ws's HTTP/3 server framework runs on the standalone crates **nyseion-quic** (QUIC transport,
its TLS on rustls), **nyseion-h3** (HTTP/3) and **nyseion-qpack** (QPACK), and the in-tree QUIC,
HTTP/3 and QPACK code is gone (`wtx/src/quic`, `wtx/src/http3` but its re-exports and the test
client, `wtx/src/tls/quic_tls.rs`). The framework's worker loop and its io_uring UDP socket are
nyseion-ws's own, as before. These public items changed.

## Server framework

- **`Http3ServerFramework::new(executor, rng, tls)` takes a `RustlsConfig`**
  (`nyseion_ws::quic::rustls::RustlsConfig`), not a `TlsConfig<TCX>`, and the framework lost its
  `TCX` type parameter: `Http3ServerFramework<DA, EC, EX, RC, RNG>`.
  - **Why:** the QUIC handshake is rustls's (nyseion-quic's adapter); wtx's `TlsConfig` cannot hand
    its private key to rustls.
  - Build it from the PEM (or DER) pair:
    `RustlsConfig::builder().cert_chain_pem(&cert).private_key_pem(&key).build()`. Its ALPN is
    `h3` unless the builder is told otherwise (before: `h3` whatever the `TlsConfig` listed).
    `build` returns `nyseion_quic::rustls::ConfigError`; `std::io::Error::other(err)` makes a
    `nyseion_ws::Error` of it.
- **`set_h3_params`**: `H3Params` is nyseion-h3's and has three more fields. The framework's
  responses follow `set_content_length`, `set_date` and `set_rfc9110_headers` as before and ignore
  `H3Params::content_length` and `H3Params::date`; `H3Params::response_encoder` is not used (the
  router's responses are encoded with the static table of QPACK).
- **`Http3WorkerStats::pool_bytes`** (test-only) no longer counts TLS handshake states: rustls
  keeps none between handshakes. The idle trim of a worker no longer trims a TLS pool.

## `nyseion_ws::quic`

- **It is the nyseion-quic crate, re-exported** (`pub use nyseion_quic as quic`): `QuicParams`,
  `QuicLimits` (same setters and fields), `QuicError`, `TransportErrorCode`, `TxArena`,
  `TxTrain`, `TrainId`, `DatagramMeta`, `Cid`, `MAX_CID_LEN`, `TransportParams`, `WorkerLoads`,
  `steering_cbpf`, `SteeringProgram`, `STEERING_PROGRAM_LEN` and `diag` keep their names, now with
  nyseion-quic's documentation and more (`Endpoint`, `QuicApp`, `ConnCtx`, `BodyRef`, `rustls`, ...).
- The hidden check modules (`frame_check`, `packet_check`, `conn_check`, `endpoint_check`) are
  nyseion-quic's, behind its `__fuzz` feature.

## `nyseion_ws::http3`

- **`H3Params`, `H3ErrorCode` and `Http3Error` are nyseion-h3's.**
  - `H3Params` has `content_length`, `date` and `response_encoder` (a struct literal needs
    `..H3Params::default()`).
  - `Http3Error` lost the QPACK reasons (`CapacityAboveMaximum`, `DecoderStreamBacklog`,
    `DecoderStreamInstruction`, `EncoderInstructionBudget`, `EntryTooLarge`, `InvalidBase`,
    `InvalidDynamicIndex`, `InvalidRequiredInsertCount`, `InvalidStaticIndex`,
    `RequiredInsertCountTooLarge`, `TooManyBlockedStreams`, `Truncated`, `IntegerOverflow`,
    `InvalidHuffman`, `HuffmanOutputTooLong`): QPACK errors are `nyseion_qpack::Error`, inside
    `nyseion_h3::Error::Qpack`. It gained `NoHead` and `OutOfMemory`.
- `http3::loopback_client` (test-only, feature `_h3-loopback-client`) keeps its API
  (`H3LoopbackClient`, `H3Get`, `H3Reply`); it runs nyseion-quic's test client.

## `nyseion_ws::Error`

- **`Error::Quic` carries `nyseion_quic::Error`** (was `QuicError`) and **`Error::Http3` carries
  `nyseion_h3::Error`** (was `Http3Error`). `Error::QuicClose` and `Error::Http3Close` are gone: they
  are `nyseion_quic::Error::QuicClose` and `nyseion_h3::Error::Http3Close` inside those.
  `From<QuicError>`, `From<nyseion_quic::Error>` and `From<nyseion_h3::Error>` exist.

## `nyseion_ws::crypto`

- **The header-protection keys are gone**: `HpKey`, `HpKeyDummy`, `Aes128HpGlobal`,
  `Aes256HpGlobal`, `Chacha20HpGlobal` and their backend wrappers (`Aes128HpRing`, `...Alr`,
  `...Ruco`, ...). Only QUIC used them; nyseion-quic has its own. The AEAD keys (`AeadKey`, ...)
  stay: TLS uses them.

## Features

- **`quic` needs `crypto-ring` or `crypto-alr`**, nyseion-quic's backends (its packet keys and
  rustls's provider). `crypto-graviola` and `crypto-ruco` alone no longer give QUIC.
- `quic` no longer turns on `tls`, `foldhash` and `hashbrown`; `http3` turns on the nyseion-h3 and
  nyseion-qpack dependencies.
- `_fuzz-conn` is gone, and so are the wtx-fuzz targets `quic_packet`, `quic_frame`,
  `quic_transport_params`, `quic_conn`, `quic_endpoint`, `h3_conn` and `qpack_decoder` (with their
  corpora and `.scripts/fuzz-h3.sh`): nyseion-quic's `fuzz/` (packet, frame, transport parameters,
  connection, endpoint, `h3_conn`, `h3_streams`) and nyseion-qpack's `fuzz/` have them.

## TLS (nyseion-ws's own, over TCP)

- Its QUIC mode is gone: the server-side handshake no longer takes or sends QUIC transport
  parameters (`negotiate_client_hello` lost its `IS_QUIC` parameter). TLS over TCP is unchanged.
  `TlsError::MissingQuicTransportParameters` stays but is no longer produced.

## Behaviour

- **The QUIC handshake is rustls's.** Resumption, session tickets and 0-RTT stay refused. A
  handshake allocates (96 allocations with ring and an RSA-2048 certificate: 82 in rustls, 5 in
  nyseion-quic's adapter, 9 for ring's signature; the in-tree handshake pooled its state and took the
  9 of the signature), and costs about 6–8 % more CPU per ECDSA handshake (nyseion-quic's
  `docs/spike-tls.md`).
- **QPACK encoder stream**: an instruction whose integer never ends is
  `QPACK_ENCODER_STREAM_ERROR` (0x0201) at once (nyseion-qpack), where nyseion-ws buffered it up to a cap
  and closed with `H3_EXCESSIVE_LOAD` (0x0107).
- **QPACK decode memo**: nyseion-qpack's memo adds entries up to its capacity (8 sections) before it
  reuses those emptied for the next connection, so a pooled connection state allocates nothing
  only once it decoded 8 sections (the in-tree memo, a fixed array, after 1).
