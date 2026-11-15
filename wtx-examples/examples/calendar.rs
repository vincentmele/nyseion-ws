//! Basic time operation.

extern crate nyseion_ws;

use nyseion_ws::calendar::{Datetime, IDuration};

fn main() -> nyseion_ws::Result<()> {
  println!(
    "ISO 8601 representation of the next 2 minutes in UTC: {}",
    Datetime::now()?.add(IDuration::from_minutes(2)?)?
  );
  Ok(())
}
