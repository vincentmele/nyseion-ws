//! High-level WebSocket server that supports the routing of paths.

extern crate tokio;
extern crate nyseion_ws;
extern crate nyseion_ws_examples;

use tokio::net::TcpStream;
use nyseion_ws::{
  collections::VectorUsize,
  http::WebSocketServerFramework,
  tls::{SkCtx, TlsConfig},
  web_socket::{OpCode, WebSocket, WebSocketPayloadOrigin},
};
use nyseion_ws_examples::{PUBLIC_KEY, SECRET_KEY, host_from_args};

type LocalWebSocket = WebSocket<(), TcpStream, SkCtx, false>;

fn main() -> nyseion_ws::Result<()> {
  let tls_config = TlsConfig::from_keys_pem(PUBLIC_KEY, SECRET_KEY)?;
  let router = (("/echo", echo),);
  WebSocketServerFramework::tokio(tls_config)?
    .set_error_cb(|err| eprintln!("Error: {err}"))
    .run_in_threads(&host_from_args(), router)
}

async fn echo(mut buffer: VectorUsize<u8>, mut ws: LocalWebSocket) -> nyseion_ws::Result<()> {
  let (mut common, mut reader, mut writer) = ws.split_mut();
  loop {
    let origin = WebSocketPayloadOrigin::Adaptive;
    let mut frame = reader.read_frame(&mut buffer, &mut common, origin).await?;
    match frame.op_code() {
      OpCode::Binary | OpCode::Text => writer.write_frame(&mut common, &mut frame).await?,
      OpCode::Close => return Ok(()),
      _ => {}
    }
  }
}
