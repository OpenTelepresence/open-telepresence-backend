use warp::{ws::Message, Filter, Rejection};

use std::{collections::HashMap, convert::Infallible, sync::Arc};
use tokio::sync::{mpsc, Mutex};
use clap::{arg, Command};
use std::net::SocketAddr;

mod handler;
mod ws;


#[derive(Debug, Clone)]
pub struct Client {
  pub client_id: String,
  pub sender: Option<mpsc::UnboundedSender<std::result::Result<Message, warp::Error>>>,
}

type Clients = Arc<Mutex<HashMap<String, Client>>>;
type Result<T> = std::result::Result<T, Rejection>;

#[tokio::main]
async fn main() {
  let matches = Command::new("signaler")
    .version("0.01")
    .arg(arg!(--port <VALUE>).default_value("8443"))
    .arg(arg!(--addr <VALUE>).default_value("127.0.0.1"))
    .get_matches();

  let clients: Clients = Arc::new(Mutex::new(HashMap::new()));

  let signal = warp::path("signal")
    .and(warp::ws())
    .and(with_clients(clients.clone()))
    .and_then(handler::ws_handler);

  let routes = signal.with(warp::cors().allow_any_origin());
  let addr = matches.get_one::<String>("addr").unwrap();
  let port = matches.get_one::<String>("port").unwrap();
  let port = port.parse::<u16>().unwrap();
  let addr: SocketAddr = format!("{}:{}", addr, port).parse().expect("Unable to parse ip address");
  println!("Starting server on {}", addr);
  warp::serve(routes).run(addr).await;

}

fn with_clients(clients: Clients) -> impl Filter<Extract = (Clients,), Error = Infallible> + Clone {
  warp::any().map(move || clients.clone())
}
