// #[macro_use] extern crate rocket;
// 
// use rocket::tokio::time::{sleep, Duration};
// 
// #[get("/hello")]
// fn hello() -> &'static str {
//     "Hello, world!"
// }
// 
// 
// #[get("/delay/<seconds>")]
// async fn delay(seconds: u64) -> String {
//     sleep(Duration::from_secs(seconds)).await;
//     format!("Waited for {} seconds", seconds)
// }
// 
// #[rocket::main]
// async fn main() -> Result<(), rocket::Error> {
//     let _rocket = rocket::build()
//         .mount("/hello", routes![hello])
//         .mount("/", routes![delay])
//         .launch()
//         .await?;
// 
//     Ok(())
// }
// 
