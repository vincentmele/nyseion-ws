//! HTTP/3 (RFC 9114) and QPACK (RFC 9204): the parameters and errors of the connections of the
//! HTTP/3 server framework, from the nyseion-h3 crate, which runs them on the QUIC transport of
//! nyseion-quic ([`crate::quic`]) and the QPACK codec of nyseion-qpack.

#[cfg(feature = "_h3-loopback-client")]
#[doc(hidden)]
pub mod loopback_client;

pub use nyseion_h3::{H3ErrorCode, H3Params, Http3Error};
