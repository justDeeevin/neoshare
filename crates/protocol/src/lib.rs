use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ToServer {
    pub kind: ToServerKind,
    pub bytes: Vec<u8>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum ToServerKind {
    Diff,
    Host(Uuid),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ToClient {
    pub kind: ToClientKind,
    pub bytes: Vec<u8>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum ToClientKind {
    State,
    Save(PathBuf),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Save {
    pub path: PathBuf,
}
