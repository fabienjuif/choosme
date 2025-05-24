use adw::gio::{self};
use std::{
    collections::HashMap,
    sync::mpsc::{self, Sender},
    thread::JoinHandle,
};
use tracing::error;

pub enum DesktopFileCommand {
    Launch(String),
}

pub fn init_desktop_files() -> (JoinHandle<()>, Sender<DesktopFileCommand>) {
    // Create a channel for sending commands to the desktop file handler
    let (tx, rx) = mpsc::channel();

    let jh = std::thread::spawn(move || {
        let app_cache: HashMap<String, gio::DesktopAppInfo> = HashMap::new();

        loop {
            match rx.recv() {
                Ok(DesktopFileCommand::Launch(app_id)) => { /* ... */ }
                Err(_) => {
                    error!("error receiving command from channel");
                    break;
                }
            }
        }
    });

    (jh, tx)
}
