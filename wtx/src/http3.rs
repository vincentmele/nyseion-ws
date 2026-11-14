//! HTTP/3 (RFC 9114) and QPACK (RFC 9204): the parameters and errors of the connections of the
//! HTTP/3 server framework, from the rimu-h3 crate, which runs them on the QUIC transport of
//! rimu-quic ([`crate::quic`]) and the QPACK codec of rimu-qpack.

#[cfg(feature = "_h3-loopback-client")]
#[doc(hidden)]
pub mod loopback_client;

pub use rimu_h3::{H3ErrorCode, H3Params, Http3Error};
