//! Sans-I/O HTTP/1.1 server core: a keep-alive connection state machine and the response writer,
//! on nyseion-http1's resumable request parser and body decoder.

mod conn;
mod response;

use crate::http::{Method, StatusCode, Version};

pub use conn::{Http1Conn, Http1ConnParts, Http1Event};
pub use response::write_response;
pub use nyseion_http1::Headers;

/// Maximum number of headers of a request.
pub const MAX_HEADERS: usize = 64;

/// How the body of a request is framed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BodyKind {
  /// No body.
  None,
  /// `Content-Length` bytes.
  ContentLength(usize),
  /// `Transfer-Encoding: chunked`.
  Chunked,
}

/// Borrowed view into the read buffer; no allocation.
#[derive(Clone, Copy, Debug)]
pub struct Http1Request<'b> {
  /// Request method.
  pub method: Method,
  /// [`Version::Http1`] for HTTP/1.0, [`Version::Http1_1`] for HTTP/1.1.
  pub version: Version,
  /// Percent-encoded path, without the query.
  pub path: &'b str,
  /// Everything after `?`, or an empty string.
  pub query: &'b str,
  /// The header fields, in order, as views into the read buffer. For an absolute-form target
  /// (`GET http://authority/path`) the value of `Host` is the target's authority, not what the
  /// header said (RFC 9112 §3.2.3).
  pub headers: Headers<'b>,
  /// Framing of the body.
  pub body_kind: BodyKind,
  /// If `false`, respond and then close the connection.
  pub keep_alive: bool,
}

/// nyseion-http1's method as nyseion-ws's. `None` for an extension method, which the parser only
/// accepts when configured to (nyseion-ws does not).
const fn method_of(method: nyseion_http1::Method) -> Option<Method> {
  Some(match method {
    nyseion_http1::Method::Connect => Method::Connect,
    nyseion_http1::Method::Delete => Method::Delete,
    nyseion_http1::Method::Get => Method::Get,
    nyseion_http1::Method::Head => Method::Head,
    nyseion_http1::Method::Options => Method::Options,
    nyseion_http1::Method::Patch => Method::Patch,
    nyseion_http1::Method::Post => Method::Post,
    nyseion_http1::Method::Put => Method::Put,
    nyseion_http1::Method::Trace => Method::Trace,
    nyseion_http1::Method::Other => return None,
  })
}

/// nyseion-http1's version as nyseion-ws's.
const fn version_of(version: nyseion_http1::Version) -> Version {
  match version {
    nyseion_http1::Version::Http10 => Version::Http1,
    nyseion_http1::Version::Http11 => Version::Http1_1,
  }
}

/// The status a parse or body error is answered with: 400, 414, 431, 501 or 505.
fn status_of(err: nyseion_http1::Error) -> StatusCode {
  match err.status() {
    414 => StatusCode::UriTooLong,
    431 => StatusCode::RequestHeaderFieldsTooLarge,
    501 => StatusCode::NotImplemented,
    505 => StatusCode::HttpVersionNotSupported,
    _ => StatusCode::BadRequest,
  }
}

/// nyseion-http1's request framing as nyseion-ws's. `None` when the length does not fit in `usize`
/// (answered like any body over the limit).
fn body_kind_of(body: nyseion_http1::BodyKind) -> Option<BodyKind> {
  Some(match body {
    nyseion_http1::BodyKind::ContentLength(len) => BodyKind::ContentLength(usize::try_from(len).ok()?),
    nyseion_http1::BodyKind::Chunked => BodyKind::Chunked,
    nyseion_http1::BodyKind::None | nyseion_http1::BodyKind::UntilClose => BodyKind::None,
  })
}

#[cfg(test)]
mod tests {
  use crate::{
    http::{HttpRecvParams, Method, StatusCode, Version},
    http1::{BodyKind, Http1Conn, body_kind_of, method_of, status_of, version_of},
  };

  #[test]
  fn scaffold_types_exist() {
    let _conn = Http1Conn::new(&HttpRecvParams::with_default_params());
    assert!(matches!(BodyKind::ContentLength(2), BodyKind::ContentLength(2)));
  }

  #[test]
  fn wire_types_map_to_nyseion_ws_types() {
    assert_eq!(method_of(nyseion_http1::Method::Patch), Some(Method::Patch));
    assert_eq!(method_of(nyseion_http1::Method::Other), None);
    assert_eq!(version_of(nyseion_http1::Version::Http10), Version::Http1);
    assert_eq!(
      status_of(nyseion_http1::Error::TooManyHeaders),
      StatusCode::RequestHeaderFieldsTooLarge
    );
    assert_eq!(status_of(nyseion_http1::Error::UnknownMethod), StatusCode::NotImplemented);
    assert_eq!(status_of(nyseion_http1::Error::InvalidHost), StatusCode::BadRequest);
    assert_eq!(
      body_kind_of(nyseion_http1::BodyKind::ContentLength(7)),
      Some(BodyKind::ContentLength(7))
    );
  }
}
