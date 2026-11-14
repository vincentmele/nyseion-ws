//! A minimal HTTP/3 client over a UDP socket, for the in-process tests of binaries (feature
//! `_h3-loopback-client`: test-only, not a public API, hidden from the documentation).
//!
//! It runs rimu-quic's test client ([`TestClient`]), with the static QPACK table only, and sends
//! `GET`s without a control stream. It is not a general client: it trusts any certificate,
//! follows no Retry and migrates nowhere. Its allocations are those of a test (a reply owns its
//! bytes), so a test that counts the allocations of a server counts the threads of the server
//! only.

use alloc::{collections::BTreeMap, string::String, vec, vec::Vec};
use core::time::Duration;
use rimu_h3::frame::{H3FrameTy, decode_frame_header, encode_frame_header};
use rimu_qpack::{DecodeOutcome, DecodedField, Decoder, DecoderConfig, encode_static};
use rimu_quic::{ConnCtx, QuicApp, QuicParams, StreamId, client::TestClient, rng::ChaCha20};
use std::{net::UdpSocket, time::Instant};

const TIMEOUT: Duration = Duration::from_secs(10);

/// A response.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct H3Reply {
  /// The payloads of the DATA frames.
  pub body: Vec<u8>,
  /// The fields of the HEADERS frame, `:status` included.
  pub fields: Vec<(Vec<u8>, Vec<u8>)>,
  /// `:status`.
  pub status: u16,
}

impl H3Reply {
  /// The value of the field `name` (lower case).
  pub fn field(&self, name: &str) -> Option<&[u8]> {
    self.fields.iter().find(|el| el.0 == name.as_bytes()).map(|el| el.1.as_slice())
  }
}

/// A request to send: its path and the fields after the four pseudo-header fields.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct H3Get {
  /// Fields after `:method`, `:scheme`, `:authority` (`localhost`) and `:path`.
  pub fields: Vec<(String, String)>,
  /// `:path`.
  pub path: String,
}

/// What arrived on a stream.
#[derive(Debug, Default)]
struct Arrived {
  data: Vec<u8>,
  fin: bool,
}

/// Collects what arrives on every stream.
#[derive(Debug, Default)]
struct ClientApp {
  closed: bool,
  streams: BTreeMap<u64, Arrived>,
}

impl QuicApp for ClientApp {
  type ConnState = ();

  fn on_established(&mut self, _: &mut ConnCtx<'_, ()>) -> rimu_quic::Result<()> {
    Ok(())
  }

  fn on_stream_data(
    &mut self,
    _: &mut ConnCtx<'_, ()>,
    id: StreamId,
    data: &[u8],
    fin: bool,
  ) -> rimu_quic::Result<()> {
    let arrived = self.streams.entry(id.0).or_default();
    arrived.data.extend_from_slice(data);
    arrived.fin |= fin;
    Ok(())
  }

  fn on_stream_reset(
    &mut self,
    _: &mut ConnCtx<'_, ()>,
    id: StreamId,
    _: u64,
  ) -> rimu_quic::Result<()> {
    // A reply that ends in a reset is never complete
    let _rslt = self.streams.remove(&id.0);
    Ok(())
  }

  fn on_stop_sending(
    &mut self,
    _: &mut ConnCtx<'_, ()>,
    _: StreamId,
    _: u64,
  ) -> rimu_quic::Result<()> {
    Ok(())
  }

  fn on_closed(&mut self, _: &mut ()) {
    self.closed = true;
  }
}

/// A client of one connection to a server on `server`.
#[derive(Debug)]
pub struct H3LoopbackClient {
  conn: TestClient<ClientApp>,
  epoch: Instant,
  rx: Vec<u8>,
  server: core::net::SocketAddr,
  socket: UdpSocket,
}

impl H3LoopbackClient {
  /// Connects to `server` and returns once the handshake is confirmed.
  ///
  /// # Errors
  ///
  /// If the handshake does not complete in ten seconds.
  pub fn connect(server: core::net::SocketAddr) -> crate::Result<Self> {
    let bind = core::net::SocketAddr::new(server.ip(), 0);
    let socket = UdpSocket::bind(bind)?;
    let mut rng = ChaCha20::from_os()?;
    let params = QuicParams::default();
    let conn = TestClient::new(ClientApp::default(), &params, server, b"h3", &mut rng, 0)?;
    let mut this = Self { conn, epoch: Instant::now(), rx: vec![0; 1 << 16], server, socket };
    let begin = Instant::now();
    while !this.conn.is_confirmed() {
      if begin.elapsed() > TIMEOUT || this.conn.app().closed {
        return Err(std::io::Error::other("the HTTP/3 handshake did not complete").into());
      }
      this.poll(Duration::from_millis(5))?;
    }
    Ok(this)
  }

  /// Closes the connection with a CONNECTION_CLOSE (code 0x100, no error) and sends it.
  ///
  /// # Errors
  ///
  /// If the datagram cannot be sent.
  pub fn close(&mut self) -> crate::Result<()> {
    let now_us = self.now_us();
    self.conn.close(now_us, 0x100);
    self.flush()
  }

  /// Sends what the connection has to send, then takes the datagrams that arrive for at most
  /// `wait` (or until its next timer) and runs the timers that are due.
  ///
  /// # Errors
  ///
  /// If the socket fails.
  pub fn poll(&mut self, wait: Duration) -> crate::Result<()> {
    self.flush()?;
    let now_us = self.now_us();
    let until_timer = self
      .conn
      .next_deadline()
      .map_or(wait, |el| Duration::from_micros(el.saturating_sub(now_us)).min(wait));
    self.socket.set_read_timeout(Some(until_timer.max(Duration::from_micros(100))))?;
    loop {
      match self.socket.recv_from(&mut self.rx) {
        Ok((len, _)) => {
          let now_us = self.now_us();
          self.conn.recv(now_us, self.rx.get_mut(..len).unwrap_or_default());
          // The rest of the burst without waiting
          self.socket.set_nonblocking(true)?;
        }
        Err(err)
          if matches!(
            err.kind(),
            std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
          ) =>
        {
          break;
        }
        Err(err) => return Err(err.into()),
      }
    }
    self.socket.set_nonblocking(false)?;
    let now_us = self.now_us();
    if self.conn.next_deadline().is_some_and(|el| el <= now_us) {
      self.conn.on_timeout(now_us);
    }
    Ok(())
  }

  /// Sends `get` on a new stream of the connection.
  ///
  /// # Errors
  ///
  /// If the connection cannot open another stream now (the server's limit: poll and try again).
  pub fn send(&mut self, get: &H3Get) -> crate::Result<u64> {
    let mut fields: Vec<(&[u8], &[u8])> = vec![
      (b":method", b"GET"),
      (b":scheme", b"https"),
      (b":authority", b"localhost"),
      (b":path", get.path.as_bytes()),
    ];
    fields.extend(get.fields.iter().map(|el| (el.0.as_bytes(), el.1.as_bytes())));
    let mut section = Vec::new();
    encode_static(fields.iter().copied(), &mut section);
    let mut frame = vec![0u8; 16];
    let len = encode_frame_header(H3FrameTy::Headers, section.len() as u64, &mut frame)
      .ok_or(crate::Error::ProgrammingError)?;
    frame.truncate(len);
    frame.extend_from_slice(&section);
    let now_us = self.now_us();
    let id = self.conn.with_ctx(now_us, |_, cx| {
      let id = cx.open_bidi()?;
      cx.write(id, &frame, true)?;
      Ok::<_, rimu_quic::Error>(id)
    })?;
    Ok(id.0)
  }

  /// The reply on stream `id`, once complete, taken out of the client.
  ///
  /// # Errors
  ///
  /// If the reply is not a well-formed response with a `:status`.
  pub fn take(&mut self, id: u64) -> crate::Result<Option<H3Reply>> {
    if !self.conn.app().streams.get(&id).is_some_and(|el| el.fin) {
      return Ok(None);
    }
    let Some(arrived) = self.conn.app_mut().streams.remove(&id) else {
      return Ok(None);
    };
    parse(id, &arrived.data).map(Some)
  }

  /// Sends `get` and waits for its reply.
  ///
  /// # Errors
  ///
  /// If there is no reply in ten seconds, or the reply is malformed.
  pub fn get(&mut self, get: &H3Get) -> crate::Result<H3Reply> {
    let id = self.send(get)?;
    let begin = Instant::now();
    loop {
      if let Some(reply) = self.take(id)? {
        return Ok(reply);
      }
      if begin.elapsed() > TIMEOUT || self.conn.app().closed {
        return Err(std::io::Error::other("no HTTP/3 reply").into());
      }
      self.poll(Duration::from_millis(5))?;
    }
  }

  /// Sends `total` requests, `request(n)` the n-th, with at most `depth` in flight, and gives each
  /// reply to `reply(n, reply)` as it completes (not in order).
  ///
  /// # Errors
  ///
  /// If the replies do not complete in ten seconds of silence, or one is malformed.
  pub fn pipeline(
    &mut self,
    total: usize,
    depth: usize,
    mut request: impl FnMut(usize) -> H3Get,
    mut reply: impl FnMut(usize, H3Reply),
  ) -> crate::Result<()> {
    let mut in_flight: Vec<(u64, usize)> = Vec::new();
    let (mut sent, mut done) = (0usize, 0usize);
    let mut last_progress = Instant::now();
    while done < total {
      while sent < total && in_flight.len() < depth {
        match self.send(&request(sent)) {
          Ok(id) => {
            in_flight.push((id, sent));
            sent = sent.wrapping_add(1);
          }
          // The server's limit of streams: wait for replies
          Err(_) if !in_flight.is_empty() => break,
          Err(err) => return Err(err),
        }
      }
      self.poll(Duration::from_millis(1))?;
      let mut nth = 0;
      while let Some(&(id, n)) = in_flight.get(nth) {
        if let Some(elem) = self.take(id)? {
          reply(n, elem);
          let _rslt = in_flight.swap_remove(nth);
          done = done.wrapping_add(1);
          last_progress = Instant::now();
        } else {
          nth = nth.wrapping_add(1);
        }
      }
      if last_progress.elapsed() > TIMEOUT || self.conn.app().closed {
        return Err(std::io::Error::other("the HTTP/3 replies stopped").into());
      }
    }
    Ok(())
  }

  /// Sends what the connection has to send and reads nothing: the datagrams that arrive stay in
  /// the socket and are not acknowledged (task 23d: a peer that withholds its ACKs).
  ///
  /// # Errors
  ///
  /// If the socket fails.
  pub fn send_pending_only(&mut self) -> crate::Result<()> {
    self.flush()
  }

  fn flush(&mut self) -> crate::Result<()> {
    let now_us = self.now_us();
    let Self { conn, server, socket, .. } = self;
    let mut rslt = Ok(());
    let _sent = conn.flush(now_us, |datagram| {
      if rslt.is_ok()
        && let Err(err) = socket.send_to(datagram, *server)
      {
        rslt = Err(err);
      }
    });
    Ok(rslt?)
  }

  fn now_us(&self) -> u64 {
    u64::try_from(self.epoch.elapsed().as_micros()).unwrap_or(u64::MAX)
  }
}

// The frames of a response: the fields of the HEADERS frame and the payloads of the DATA frames.
fn parse(id: u64, bytes: &[u8]) -> crate::Result<H3Reply> {
  let bad = || crate::Error::from(std::io::Error::other("a malformed HTTP/3 response"));
  let mut reply = H3Reply::default();
  let mut decoder = Decoder::new(DecoderConfig {
    max_table_capacity: 0,
    blocked_streams: 0,
    memo_sections: 0,
    ..DecoderConfig::default()
  });
  let mut rest = bytes;
  while let Some((ty, len, header_len)) = decode_frame_header(rest)? {
    let len = usize::try_from(len).map_err(|_| bad())?;
    let payload = rest.get(header_len..header_len.wrapping_add(len)).ok_or_else(bad)?;
    match ty {
      H3FrameTy::Headers => {
        let mut fields = Vec::new();
        let outcome = decoder
          .decode(id, payload, |field: DecodedField<'_>| {
            fields.push((field.name.to_vec(), field.value.to_vec()));
            Ok::<(), rimu_qpack::Error>(())
          })
          .map_err(|_| bad())?;
        if !matches!(outcome, DecodeOutcome::Done { .. }) {
          return Err(bad());
        }
        let status = fields.iter().find(|el| el.0 == b":status").ok_or_else(bad)?;
        let text = crate::misc::from_utf8_basic(&status.1).ok();
        reply.status = text.and_then(|el| el.parse().ok()).ok_or_else(bad)?;
        reply.fields = fields;
      }
      H3FrameTy::Data => reply.body.extend_from_slice(payload),
      _ => {}
    }
    rest = rest.get(header_len.wrapping_add(len)..).ok_or_else(bad)?;
  }
  if rest.is_empty() { Ok(reply) } else { Err(bad()) }
}
