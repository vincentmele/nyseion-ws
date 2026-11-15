#[nyseion_ws::pkg]
mod a {
}

#[nyseion_ws::pkg]
mod b {
  #[pkg::req_data]
  struct Req(
    i32
  );
}

#[nyseion_ws::pkg]
mod c {
  #[pkg::res_data]
  struct Res;
}

fn main() {
}
