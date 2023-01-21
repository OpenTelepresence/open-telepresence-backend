use warp::{ws::Message, Filter, Rejection};

use std::{collections::HashMap, convert::Infallible, sync::Arc};
use tokio::sync::{mpsc, Mutex};
use clap::{arg, Command};
use std::net::SocketAddr;

use log::start_logger;
use tracing::{info, debug};

mod log;
mod handler;
mod ws;
mod webrtc;
use crate::webrtc::{WebRTCConnection, Track};



#[derive(Debug, Clone)]
pub struct Group {
  pub clients: Arc<Mutex<Vec<Arc<Mutex<Client>>>>>,
}


#[derive(Debug, Clone)]
pub struct Client {
  pub client_id: String,
  pub peer_connection: Option<Box<webrtc::WebRTCConnection>>,
  pub sender: Arc<Box<mpsc::UnboundedSender<std::result::Result<Message, warp::Error>>>>,
  pub group: Option<Arc<Group>>,
}

impl Group {
  pub fn new() -> Group {
    let clients = Arc::new(Mutex::new(Vec::new()));
    Group { clients }
  }

  pub async fn subscribe(&mut self, client: Arc<Mutex<Client>>) {
    self.clients.lock().await.push(client);
  }

  pub async fn add_tracks(&self, to_peer: &mut Box<WebRTCConnection>) {
    let clients = self.clients.lock().await;
    for client in clients.iter() {
      debug!("Trying client {:?}", client);
      if let Some(pc) = &client.lock().await.peer_connection {
        debug!("Got peer_connection {:?}", pc);
        for track in  pc.get_tracks().lock().await.values(){
          to_peer.add_remote_track(&track).await;
          debug!("Adding track {:?} to peer {:?}\n", track, to_peer.get_id());
        }
      }
    }
  }


  pub async fn notify_track(&mut self, track: &Arc<Track>) {
    let mut clients = self.clients.lock().await;
    for client in clients.iter_mut() {
      if let Some(pc) = &mut client.lock().await.peer_connection {
        pc.add_remote_track(&track).await;
      }
    }
  }
}


type Clients = Arc<Mutex<HashMap<String, Arc<Mutex<Client>>>>>;
type Groups = Arc<Mutex<HashMap<String, Arc<Mutex<Group>>>>>;
type Result<T> = std::result::Result<T, Rejection>;

#[tokio::main]
async fn main() {
  let matches = Command::new("signaler")
    .version("0.01")
    .arg(arg!(--port <VALUE>).default_value("9999").required(false))
    .arg(arg!(--addr <VALUE>).default_value("127.0.0.1").required(false))
    .get_matches();

  start_logger();

  let clients: Clients = Arc::new(Mutex::new(HashMap::new()));
  let groups: Groups = Arc::new(Mutex::new(HashMap::new()));

  let signal = warp::path("signal")
    .and(warp::ws())
    .and(with_clients(clients.clone()))
    .and(with_groups(groups.clone()))
    .and_then(handler::ws_handler);

  let routes = signal.with(warp::cors().allow_any_origin());
  let addr = matches.get_one::<String>("addr").unwrap();
  let port = matches.get_one::<String>("port").unwrap();
  let port = port.parse::<u16>().unwrap();
  let addr: SocketAddr = format!("{}:{}", addr, port).parse().expect("Unable to parse ip address");
  info!("Starting server on {}", addr);
  warp::serve(routes).run(addr).await;

}

fn with_groups(groups: Groups) -> impl Filter<Extract = (Groups,), Error = Infallible> + Clone {
  warp::any().map(move || groups.clone())
}

fn with_clients(clients: Clients) -> impl Filter<Extract = (Clients,), Error = Infallible> + Clone {
  warp::any().map(move || clients.clone())
}
