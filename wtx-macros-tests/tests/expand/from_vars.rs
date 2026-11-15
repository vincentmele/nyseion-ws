extern crate alloc;

use nyseion_ws::collections::VectorUsize;

#[derive(nyseion_ws::FromVars)]
pub struct Foo {
  bar0: String,
  bar1: Option<String>,
  #[from_vars(parse_bar2)]
  bar2: u16,
  #[from_vars(parse_bar3)]
  bar3: Option<u32>,
  bar4: VectorUsize<u8>
}

fn parse_bar2(value: String) -> nyseion_ws::Result<u16> {
    Ok(value.parse()?)
}

fn parse_bar3(value: String) -> nyseion_ws::Result<u32> {
    Ok(value.parse()?)
}

fn main() {}