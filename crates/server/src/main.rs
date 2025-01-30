use std::{collections::HashMap, sync::Arc};

use futures::{SinkExt, StreamExt};
use tokio::sync::{mpsc, RwLock};
use uuid::Uuid;
use warp::{
    filters::ws::{Message, WebSocket, Ws},
    Filter,
};
use yrs::{
    updates::{decoder::Decode, encoder::Encode},
    Doc, ReadTxn, Transact, Update,
};

type Clients = Arc<RwLock<HashMap<Uuid, mpsc::UnboundedSender<Message>>>>;

#[tokio::main]
async fn main() {
    let doc = Arc::new(Doc::new());
    let clients = Arc::new(RwLock::new(HashMap::new()));
    let root = std::env::args().nth(1).unwrap_or(".".to_string());

    let join_route = warp::path("join")
        .and(warp::ws())
        .and(warp::any().map(move || doc.clone()))
        .and(warp::any().map(move || clients.clone()))
        .map(|ws: Ws, doc, clients| {
            ws.on_upgrade(move |socket| handle_connection(socket, doc, clients))
        });
    let file_route = warp::path("file").and(warp::get()).and(warp::fs::dir(root));

    warp::serve(join_route.or(file_route))
        .run(([127, 0, 0, 1], 8080))
        .await;
}

async fn handle_connection(socket: WebSocket, doc: Arc<Doc>, clients: Clients) {
    let (mut ws_tx, mut ws_rx) = socket.split();

    let state = serde_json::to_string(&doc.transact().state_vector().encode_v2()).unwrap();
    let _ = ws_tx.send(Message::text(state)).await;

    let (tx, mut rx) = mpsc::unbounded_channel();
    tokio::spawn(async move {
        while let Some(message) = rx.recv().await {
            if ws_tx.send(message).await.is_err() {
                return;
            }
        }
    });
    let id = Uuid::new_v4();
    clients.write().await.insert(id, tx.clone());

    while let Some(Ok(msg)) = ws_rx.next().await {
        if let Ok(text) = msg.to_str() {
            let Ok(diff) = Update::decode_v2(&serde_json::from_str::<Vec<u8>>(text).unwrap())
            else {
                let _ = tx.send(Message::text("Invalid diff"));
                continue;
            };

            doc.transact_mut().apply_update(diff).unwrap();
            for (sub_id, sub_tx) in clients.read().await.iter() {
                if *sub_id == id {
                    continue;
                }
                let _ = sub_tx.send(msg.clone());
            }
        }
    }

    clients.write().await.remove(&id);
}
