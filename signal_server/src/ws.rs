use crate::{Client, Clients};
use futures::{FutureExt, StreamExt};
use tokio::sync::mpsc;
use tokio_stream::wrappers::UnboundedReceiverStream;
use uuid::Uuid;
use warp::ws::{Message, WebSocket};

pub async fn client_connection(ws: WebSocket, clients: Clients) {
  let (client_ws_sender, mut client_ws_rcv) = ws.split();
  let (client_sender, client_rcv) = mpsc::unbounded_channel();

  let client_rcv = UnboundedReceiverStream::new(client_rcv);

  tokio::task::spawn(client_rcv.forward(client_ws_sender).map(|result| {
    if let Err(e) = result {
      println!("error sending websocket msg: {}", e);
    }
  }));

  let uuid = Uuid::new_v4().as_simple().to_string();

  let new_client = Client {
    client_id: uuid.clone(),
    sender: Some(client_sender)
  };

  clients.lock().await.insert(uuid.clone(), new_client);

  while let Some(result) = client_ws_rcv.next().await {
    let msg = match result {
      Ok(msg) => msg,
      Err(e) => {
        println!("error receiving message for id {}): {}", uuid.clone(), e);
        break;
      }
    };
    client_msg(&uuid, msg, &clients).await;
  }

  clients.lock().await.remove(&uuid);

  println!("{} disconnected", uuid);
}

async fn client_msg(client_id: &str, msg: Message, clients: &Clients) {
  println!("received message from {}: {:?}", client_id, msg);
  let message = match msg.to_str() {
    Ok(v) => v,
    Err(_) => return,
  };
  // let's broadcast for now to all clients
  for (uuid, client) in clients.lock().await.iter() {
    if uuid == client_id {
      continue;
    }
    if let Some(sender) = &client.sender {
      println!("broadcasting message to {}", uuid);
      let _ = sender.send(Ok(msg.clone()));
    }
  }
}
