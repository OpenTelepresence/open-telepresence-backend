use std::sync::Arc;

use crate::{Client, Clients, Group, Groups};
use futures::{FutureExt, StreamExt};
use tokio::sync::{mpsc, Mutex};
use tokio_stream::wrappers::UnboundedReceiverStream;
use uuid::Uuid;
use warp::ws::{Message, WebSocket};
use crate::webrtc::WebRTCConnection;
use serde_json::Value;
use tracing::{info, debug, warn, error};

pub async fn client_connection(ws: WebSocket, clients: Clients, groups: Groups) {
  let (client_ws_sender, mut client_ws_rcv) = ws.split();
  let (client_sender, client_rcv) = mpsc::unbounded_channel();

  let client_rcv = UnboundedReceiverStream::new(client_rcv);

  // Forward messages fro mthe unbounded_channelto the websocket connection
  tokio::task::spawn(client_rcv.forward(client_ws_sender).map(|result| {
    if let Err(e) = result {
      warn!("error sending websocket msg: {}", e);
    }
  }));

  let uuid = Uuid::new_v4().as_simple().to_string();
  let client_sender = Arc::new(Box::new(client_sender));
  let new_client = Client {
    client_id: uuid.clone(),
    peer_connection: None,
    sender: client_sender,
    group: None
  };

  clients.lock().await.insert(uuid.clone(), Arc::new(Mutex::new(new_client)));

  while let Some(result) = client_ws_rcv.next().await {
    let msg = match result {
      Ok(msg) => msg,
      Err(e) => {
        warn!("error receiving message for id {}): {}", uuid.clone(), e);
        break;
      }
    };
    client_msg(&uuid, msg, &clients, &groups).await;
  }

  clients.lock().await.remove(&uuid);

  info!("{} disconnected", uuid);
}

// msg structure
// {
//  stream-group: ...,
//  ...
// }
struct Headers {
  stream_group: Option<String>
}

async fn parse_headers(msg: &Value, groups: &Groups) -> Headers {

  let mut group_id: Option<String> = None;
  if let Some(headers) = msg.get("headers") {
    if let Some(group) = headers.get("stream-group") {
      let id = group.as_str().unwrap().to_string();
      let mut groups = groups.lock().await;
      if !groups.contains_key(&id){
        let group = Arc::new(Mutex::new(Group::new()));
        groups.insert(id.clone(), group);
      }
      group_id = Some(id);
    }
  }
  Headers {stream_group: group_id}
}

async fn client_msg(client_id: &str, msg: Message, clients: &Clients, groups: &Groups) {
  info!("received message from {}", client_id);
  debug!("msg is:\n{:?}", msg);
  let message_str = match msg.to_str() {
    Ok(v) => v,
    Err(_) => return,
  };
  debug!("Current state of groups {:?}\n", groups);
  let message: Value = serde_json::from_str(message_str).unwrap();

  let headers = parse_headers(&message, groups).await;

  let clients =  clients.lock().await;
  let client = clients.get(client_id);
  let client = client.unwrap();

  if let Some(sdp_message) = message.get("sdp") {
    match sdp_message.get("type") {
      Some(sdp_message_type) => {
        // create peer_connection
        match sdp_message_type.as_str().unwrap() {
          "offer" => {
            info!("Got offer from client {}", client_id);
            let group_id = match headers.stream_group {
              Some(v) => v,
              None => panic!("It is Forbidden to process client without a group assigned")
            };
            let group = groups.lock().await;
            let group = group.get(&group_id).unwrap().clone();
            // FIXME: cloning the mutex with client is potentially dangerous ad &Clients and
            // Groups[group] contains the client.
            group.lock().await.subscribe(client.clone()).await;
            debug!("State of group after subscribe {:?}", group);
            let mut peer_connection = match WebRTCConnection::new(client.lock().await.sender.clone(), group.clone()).await {
              Ok(conn) => {
                debug!("Successfull WebRTCConnection created {:?}", conn); 
                conn
              },
              Err(err) => {
                panic!("{:?}", err);
              }
            };
            peer_connection.setup_callbacks().await;
            group.lock().await.add_tracks(&mut peer_connection).await;
            client.lock().await.peer_connection = Some(peer_connection);
            let pc = &mut client.lock().await;
            let pc = &mut pc.peer_connection.as_mut().unwrap();

            pc.process_offer(sdp_message.to_string()).await;
            pc.summary();
          },
          "answer" => {
            if let Some(pc) = &client.lock().await.peer_connection {
              pc.process_answer(sdp_message.to_string()).await;
            }
            debug!("Got sdp answer from client {}", client_id);
          },
          _ => {
            debug!("Unkown SDP message type {:?}", sdp_message_type.as_str().unwrap());
          }
        }
      }, 
      None => {
        debug!("SDP message doesn't have a type entry: {:?}", sdp_message);
      }
    }
  }

  if let Some(ice_candidate) = message.get("ice") {
    info!("Got ice candidate from client {}", client_id);
    let client = clients.get(client_id).unwrap().lock().await;
    match &client.peer_connection {
      Some(pc) => {
        info!("Processing new ice cadidate for client {:?}", client_id);
        pc.process_ice_candidate(ice_candidate.to_string()).await;
        pc.summary();
      },
      None => error!("couldn't add ice candidate to client as it didn't have a webrtc connection")
    }
  }
}
