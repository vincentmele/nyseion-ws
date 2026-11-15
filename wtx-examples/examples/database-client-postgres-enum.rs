//! ```sql
//! CREATE TYPE custom_enum AS ENUM ('foo', 'bar', 'baz');
//! CREATE TABLE custom_enum_table (id INT, custom_enum CUSTOM_ENUM);
//! ```

extern crate tokio;
extern crate nyseion_ws;
extern crate nyseion_ws_examples;

use nyseion_ws::{
  codec::{Decode, Encode},
  database::{
    DbClient as _, Record, Typed,
    client::postgres::{Postgres, PostgresDecodeWrapper, PostgresEncodeWrapper, Ty},
  },
};

#[tokio::main]
async fn main() -> nyseion_ws::Result<()> {
  let uri = "postgres://USER:PASSWORD@localhost/DATABASE";
  let mut executor = nyseion_ws_examples::postgres_client(uri).await?;
  executor
    .execute_stmt_none("INSERT INTO custom_enum_table VALUES ($1, $2)", (1, Enum::Bar))
    .await?;
  let record = executor.execute_stmt_single("SELECT * FROM custom_enum_table;", ()).await?;
  assert_eq!(record.decode::<_, i32>(0)?, 1);
  assert_eq!(record.decode::<_, Enum>(1)?, Enum::Bar);
  Ok(())
}

#[derive(Debug, PartialEq)]
enum Enum {
  Foo,
  Bar,
  Baz,
}

impl Decode<'_, Postgres<nyseion_ws::Error>> for Enum {
  #[inline]
  fn decode(dw: &mut PostgresDecodeWrapper<'_, '_>) -> Result<Self, nyseion_ws::Error> {
    let string = <&str as Decode<Postgres<nyseion_ws::Error>>>::decode(dw)?;
    Ok(match string {
      "foo" => Self::Foo,
      "bar" => Self::Bar,
      "baz" => Self::Baz,
      _ => return Err(nyseion_ws::Error::UnexpectedString { length: 3 }),
    })
  }
}

impl Encode<Postgres<nyseion_ws::Error>> for Enum {
  #[inline]
  fn encode(&self, ew: &mut PostgresEncodeWrapper<'_>) -> Result<(), nyseion_ws::Error> {
    let string = match self {
      Self::Foo => "foo",
      Self::Bar => "bar",
      Self::Baz => "baz",
    };
    <_ as Encode<Postgres<nyseion_ws::Error>>>::encode(&string, ew)?;
    Ok(())
  }
}

impl Typed<Postgres<nyseion_ws::Error>> for Enum {
  #[inline]
  fn runtime_ty(&self) -> Option<Ty> {
    None
  }

  #[inline]
  fn static_ty() -> Option<Ty> {
    None
  }
}
