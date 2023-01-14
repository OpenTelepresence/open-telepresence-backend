use crate::{ws, Result, Clients, Groups};
use warp::Reply;

pub async fn ws_handler(ws: warp::ws::Ws, clients: Clients, groups: Groups) -> Result<impl Reply> {
    println!("ws_handler");

    Ok(ws.on_upgrade(move |socket| ws::client_connection(socket, clients, groups)))
}
