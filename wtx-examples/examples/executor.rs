//! The `executor` feature allows the execution of asynchronous operations

extern crate nyseion_ws;

#[nyseion_ws::main]
async fn main() {
  println!("With great power comes great electricity bills");
}

#[nyseion_ws::test]
async fn test_with_runtime(runtime: &nyseion_ws::executor::StdRuntime) {
  runtime
    .spawn(async move {
      println!("Behind every successful diet is an unwatched pizza");
    })
    .unwrap();
}
