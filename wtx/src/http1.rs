//! Sans-I/O HTTP/1.1 server core: a keep-alive connection state machine and the response writer,
//! on rimu-http1's resumable request parser and body decoder.

mod conn;
mod response;

use crate::http::{Method, StatusCode, Version};

pub use conn::{Http1Conn, Http1ConnParts, Http1Event};
pub use response::write_response;
pub use rimu_http1::Headers;

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

/// rimu-http1's method as rimu-ws's. `None` for an extension method, which the parser only
/// accepts when configured to (rimu-ws does not).
const fn method_of(method: rimu_http1::Method) -> Option<Method> {
  Some(match method {
    rimu_http1::Method::Connect => Method::Connect,
    rimu_http1::Method::Delete => Method::Delete,
    rimu_http1::Method::Get => Method::Get,
    rimu_http1::Method::Head => Method::Head,
    rimu_http1::Method::Options => Method::Options,
    rimu_http1::Method::Patch => Method::Patch,
    rimu_http1::Method::Post => Method::Post,
    rimu_http1::Method::Put => Method::Put,
    rimu_http1::Method::Trace => Method::Trace,
    rimu_http1::Method::Other => return None,
  })
}

/// rimu-http1's version as rimu-ws's.
const fn version_of(version: rimu_http1::Version) -> Version {
  match version {
    rimu_http1::Version::Http10 => Version::Http1,
    rimu_http1::Version::Http11 => Version::Http1_1,
  }
}

/// The status a parse or body error is answered with: 400, 414, 431, 501 or 505.
fn status_of(err: rimu_http1::Error) -> StatusCode {
  match err.status() {
    414 => StatusCode::UriTooLong,
    431 => StatusCode::RequestHeaderFieldsTooLarge,
    501 => StatusCode::NotImplemented,
    505 => StatusCode::HttpVersionNotSupported,
    _ => StatusCode::BadRequest,
  }
}

/// rimu-http1's request framing as rimu-ws's. `None` when the length does not fit in `usize`
/// (answered like any body over the limit).
fn body_kind_of(body: rimu_http1::BodyKind) -> Option<BodyKind> {
  Some(match body {
    rimu_http1::BodyKind::ContentLength(len) => BodyKind::ContentLength(usize::try_from(len).ok()?),
    rimu_http1::BodyKind::Chunked => BodyKind::Chunked,
    rimu_http1::BodyKind::None | rimu_http1::BodyKind::UntilClose => BodyKind::None,
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
  fn wire_types_map_to_rimu_ws_types() {
    assert_eq!(method_of(rimu_http1::Method::Patch), Some(Method::Patch));
    assert_eq!(method_of(rimu_http1::Method::Other), None);
    assert_eq!(version_of(rimu_http1::Version::Http10), Version::Http1);
    assert_eq!(
      status_of(rimu_http1::Error::TooManyHeaders),
      StatusCode::RequestHeaderFieldsTooLarge
    );
    assert_eq!(status_of(rimu_http1::Error::UnknownMethod), StatusCode::NotImplemented);
    assert_eq!(status_of(rimu_http1::Error::InvalidHost), StatusCode::BadRequest);
    assert_eq!(
      body_kind_of(rimu_http1::BodyKind::ContentLength(7)),
      Some(BodyKind::ContentLength(7))
    );
  }
}
