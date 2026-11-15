//! Long lived secret

extern crate nyseion_ws;

use std::{env, sync::OnceLock};
use nyseion_ws::secret::{Secret, SecretStr};

static SECRET: OnceLock<SecretStr> = OnceLock::new();

fn main() -> nyseion_ws::Result<()> {
  let mut data = env::args()
    .nth(1)
    .ok_or_else(|| nyseion_ws::Error::GenericStatic("No data".try_into().unwrap_or_default()))?;
  let secret = Secret::new(data.as_mut_str())?;
  let _rslt = SECRET.set(secret);
  std::thread::spawn(|| {
    let _bytes = SECRET.wait().peek()?;
    // Make API requests, decrypt AES, sign documents, do a flip, etc...
    nyseion_ws::Result::Ok(())
  })
  .join()??;
  Ok(())
}
