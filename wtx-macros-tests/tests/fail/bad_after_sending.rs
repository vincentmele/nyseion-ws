#[nyseion_ws::pkg(data_format(json), id(FooId))]
mod pkg {
  #[pkg::after_sending]
  async fn after_sending(foo: i32) -> nyseion_ws::Result<()> {
    Ok(())
  }

  #[pkg::req_data]
  struct Req;

  #[pkg::res_data]
  struct Res;
}

fn main() {
}
