use warp::{ws::Message, Filter, Rejection};

use std::{collections::HashMap, convert::Infallible, sync::Arc};
use tokio::sync::{mpsc, Mutex};

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
  let clients: Clients = Arc::new(Mutex::new(HashMap::new()));

  let signal = warp::path("signal")
    .and(warp::ws())
    .and(with_clients(clients.clone()))
    .and_then(handler::ws_handler);

  let routes = signal.with(warp::cors().allow_any_origin());
  warp::serve(routes).run(([127,0,0,1], 8443)).await;

}

fn with_clients(clients: Clients) -> impl Filter<Extract = (Clients,), Error = Infallible> + Clone {
  warp::any().map(move || clients.clone())
}
