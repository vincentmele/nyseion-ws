//! The `CorsMiddleware` middleware inserts permissive CORS headers in every response.

use nyseion_ws::{
  executor::UringExecutor,
  http::http2_server_framework::{CorsMiddleware, Http2ServerFramework, HttpRouter, get},
  rng::{ChaCha20, CryptoSeedableRng as _},
  tls::TlsConfig,
};
use nyseion_ws_examples::{PUBLIC_KEY, SECRET_KEY, host_from_args};

fn main() -> nyseion_ws::Result<()> {
  let rng = ChaCha20::from_std_random()?;
  let tls_config = TlsConfig::from_keys_pem(PUBLIC_KEY, SECRET_KEY)?;
  let router = HttpRouter::new(nyseion_ws::paths!(("/hello", get(hello))), CorsMiddleware::permissive())?;
  Http2ServerFramework::new(UringExecutor::default(), rng, tls_config)?
    .set_error_cb(|err| eprintln!("Error: {err}"))
    .run_in_threads(&host_from_args(), router)
}

async fn hello() -> &'static str {
  "Hello"
}
