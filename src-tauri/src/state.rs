use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::pty_manager::PtyManagerHandle;
use crate::status_parser::StatusTracker;
use crate::status_server::StatusServer;

pub struct AppState {
    pub pty: PtyManagerHandle,
    pub status_server: StatusServer,
    pub status_trackers: Arc<Mutex<HashMap<String, StatusTracker>>>,
    pub persistence_dir: PathBuf,
    pub persistence_lock: Mutex<()>,
}
