use crate::pty_manager::PtyManagerHandle;
use crate::status_server::StatusServer;

pub struct AppState {
    pub pty: PtyManagerHandle,
    pub status_server: StatusServer,
}
