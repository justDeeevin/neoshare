use std::{collections::HashMap, path::PathBuf, sync::Arc};

use clap::Parser;
use futures::{SinkExt, StreamExt};
use neoshare_protocol::{self as protocol, Save};
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

#[derive(clap::Parser)]
#[command(version, about)]
struct Args {
    /// Workspace root
    #[arg(short, long, default_value = ".")]
    root: PathBuf,

    /// Hosting key
    #[arg(long)]
    host_key: Uuid,
}

#[tokio::main]
async fn main() {
    let doc = Arc::new(Doc::new());
    let clients = Arc::new(RwLock::new(HashMap::new()));
    let args = Args::parse();
    let host = Arc::new(RwLock::new(None));

    let join_doc = doc.clone();
    let join_clients = clients.clone();
    let join_host = host.clone();
    let join_route = warp::path("join")
        .and(warp::ws())
        .and(warp::any().map(move || join_doc.clone()))
        .and(warp::any().map(move || join_clients.clone()))
        .and(warp::any().map(move || join_host.clone()))
        .map(move |ws: Ws, doc, clients, host| {
            ws.on_upgrade(move |socket| {
                handle_connection(socket, doc, clients, args.host_key, host)
            })
        });

    let file_route = warp::path("file").and(warp::fs::dir(args.root));

    let save_route = warp::path("save")
        .and(warp::post())
        .and(warp::body::json())
        .and(warp::any().map(move || doc.clone()))
        .and(warp::any().map(move || clients.clone()))
        .and(warp::any().map(move || host.clone()))
        .map(
            |body: Save, doc: Arc<Doc>, clients: Clients, host: Arc<RwLock<Option<Uuid>>>| {
                tokio::spawn(async move {
                    let msg = protocol::ToClient {
                        kind: protocol::ToClientKind::Save(body.path),
                        bytes: doc.transact().state_vector().encode_v2(),
                    };
                    if let Some(id) = *host.read().await {
                        let _ = clients
                            .write()
                            .await
                            .get(&id)
                            .unwrap()
                            .send(Message::text(serde_json::to_string(&msg).unwrap()));
                    }
                });
                ""
            },
        );

    warp::serve(join_route.or(file_route).or(save_route))
        .run(([127, 0, 0, 1], 8080))
        .await;
}

async fn handle_connection(
    socket: WebSocket,
    doc: Arc<Doc>,
    clients: Clients,
    master: Uuid,
    host: Arc<RwLock<Option<Uuid>>>,
) {
    let (mut ws_tx, mut ws_rx) = socket.split();

    let state_msg = protocol::ToClient {
        kind: protocol::ToClientKind::State,
        bytes: doc.transact().state_vector().encode_v2(),
    };
    let _ = ws_tx
        .send(Message::text(serde_json::to_string(&state_msg).unwrap()))
        .await;

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
            let message: protocol::ToServer = serde_json::from_str(text).unwrap();
            match message.kind {
                protocol::ToServerKind::Host(key) => {
                    if key != master {
                        let _ = tx.send(Message::text("Invalid host key"));
                        continue;
                    }
                    if host.read().await.is_some() {
                        let _ = tx.send(Message::text("Host already set"));
                        continue;
                    }
                    *host.write().await = Some(id);
                }
                protocol::ToServerKind::Diff => {
                    let Ok(diff) = Update::decode_v2(&message.bytes) else {
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
        }
    }

    clients.write().await.remove(&id);
    if id == host.read().await.unwrap() {
        for tx in clients.write().await.values() {
            let _ = tx.send(Message::close());
        }
    }
}
