use std::{
    rc::Rc,
    sync::{
        mpsc::{Receiver, Sender},
        Arc, RwLock,
    },
    thread,
};

use neoshare_protocol::{ToClient, ToServer, ToServerKind};
use nvim_oxi::{
    api::{
        self,
        opts::CreateCommandOpts,
        types::{CommandArgs, CommandNArgs, LogLevel},
    },
    plugin, print, Dictionary, Function, Object,
};
use semaphore::Semaphore;
use tungstenite::{connect, Message};
use uuid::Uuid;

#[plugin]
fn neoshare() -> nvim_oxi::Result<Dictionary> {
    let (tx, ws_recv) = std::sync::mpsc::channel();
    let (ws_send, rx) = std::sync::mpsc::channel();
    let rx = Rc::new(Semaphore::new(1, rx));

    let cmd_tx = tx.clone();
    let cmd_rx = rx.clone();
    api::create_user_command(
        "Neoshare",
        move |args| {
            let Ok(rx) = cmd_rx.try_access() else {
                error("Session already started")?;
                return Ok(());
            };
            cmd(args, cmd_tx.clone(), &rx)
        },
        &CreateCommandOpts::builder()
            .desc("Starts a neoshare session")
            .nargs(CommandNArgs::OneOrMore)
            .build(),
    )?;

    Ok(Dictionary::from_iter([(
        "start",
        Function::from(move |path: Option<String>| {
            let Ok(rx) = rx.try_access() else {
                let _ = error("Session already started");
                return;
            };

            start(path, tx.clone(), &rx);
        }),
    )]))
}

fn start(path: Option<impl Into<String>>, tx: Sender<ToClient>, rx: &Receiver<ToServer>) {
    let id = Uuid::new_v4();

    let mut cmd = std::process::Command::new("neoshare-server");
    cmd.args(["--host-key", &id.to_string()]);

    let mut alert = String::from("Started session");

    if let Some(args) = path {
        let args = args.into();
        alert.push_str(&format!(" at \"{}\"", args));
        cmd.arg(args);
    };

    std::thread::spawn(move || {
        cmd.spawn().unwrap().wait().unwrap();
    });

    alert.push('.');

    print!("{alert}");

    let (mut socket, _) = connect("ws://localhost:8080").unwrap();
    let key = Uuid::new_v4();
    let host_message = ToServer {
        kind: ToServerKind::Host(key),
        bytes: Vec::new(),
    };
    socket
        .send(Message::text(serde_json::to_string(&host_message).unwrap()))
        .unwrap();
    let socket = Arc::new(RwLock::new(socket));
    thread::spawn(move || {
        while let Ok(msg) = socket.clone().write().unwrap().read() {
            if let Ok(msg) = msg.to_text() {
                let msg: ToClient = serde_json::from_str(msg).unwrap();
                tx.send(msg).unwrap();
            }
        }
    });
    thread::spawn(move || {
        while let Ok(msg) = rx.recv() {
            let msg = serde_json::to_string(&msg).unwrap();
            socket
                .clone()
                .write()
                .unwrap()
                .send(Message::text(msg))
                .unwrap();
        }
    });
}

fn cmd(args: CommandArgs, tx: Sender<ToClient>, rx: &Receiver<ToServer>) -> nvim_oxi::Result<()> {
    match args.fargs[0].as_str() {
        "start" => start(args.fargs.get(1), tx, rx),
        cmd => {
            error(format!("Unknown command \"{cmd}\""))?;
        }
    }

    Ok(())
}

fn error(e: impl Into<String>) -> nvim_oxi::Result<Object> {
    Ok(api::notify(&e.into(), LogLevel::Error, &Dictionary::new())?)
}
