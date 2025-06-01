use gdk4::gio::AppLaunchContext;
use gtk4::gio::{DesktopAppInfo, prelude::AppInfoExt};
use std::{
    collections::HashMap,
    env,
    path::PathBuf,
    sync::mpsc::{self, Sender},
    thread::JoinHandle,
};
use tracing::{debug, error, info, warn};

use crate::config::Config;

#[derive(Debug)]
pub struct OpenParams {
    pub uris: Vec<String>,

    /// The name of the desktop file to launch.
    /// It has to be resolved beforce sending the Launch command. (Via UI for example).
    pub desktop_file_id: String,
}

pub enum DesktopFileOpenerCommand {
    /// Open a desktop file by its name.
    Open(OpenParams),

    /// Quit.
    Quit,
}

pub fn run_desktop_file_opener(cfg: Config) -> (JoinHandle<()>, Sender<DesktopFileOpenerCommand>) {
    let (tx, rx) = mpsc::channel();

    let jh = std::thread::spawn(move || {
        let desktop_files = resolve_desktop_files(&cfg);
        debug!("config is parsed and desktop files are resolved");

        loop {
            match rx.recv() {
                Ok(DesktopFileOpenerCommand::Quit) => {
                    info!("received command to quit desktop file opener");
                    break;
                }
                Ok(DesktopFileOpenerCommand::Open(params)) => {
                    info!(
                        "received command to open desktop file with params: {:?}",
                        params
                    );

                    let uris = params
                        .uris
                        .iter()
                        .map(|s| s.as_str())
                        .collect::<Vec<&str>>();

                    let Some(desktop_file) = desktop_files.get(&params.desktop_file_id) else {
                        error!("no desktop file found for id: {}", params.desktop_file_id);
                        return;
                    };

                    // open
                    if let Err(e) = desktop_file.launch_uris(&uris, None::<&AppLaunchContext>) {
                        error!(
                            "failed to open desktop file '{}': {}",
                            params.desktop_file_id, e
                        );
                    }
                }
                Err(_) => {
                    error!("error receiving command from init_desktop_files channel");
                    break;
                }
            }
        }
    });

    (jh, tx)
}

pub fn resolve_desktop_files(config_file: &Config) -> HashMap<String, DesktopAppInfo> {
    let home_dir_str = env::var("HOME").or_else(|_| env::var("USERPROFILE")).ok();

    let mut res = HashMap::new();
    for file in config_file.desktop_files.iter() {
        let desktop_file_path_str = &file.path;
        let mut desktop_file_path_buf = PathBuf::from(desktop_file_path_str);

        if let Some(end) = desktop_file_path_str.strip_prefix("~/") {
            if let Some(h_dir_path_str) = home_dir_str.as_ref() {
                let mut h_dir_path_buf = PathBuf::from(h_dir_path_str);
                h_dir_path_buf.push(end);
                desktop_file_path_buf = h_dir_path_buf;
            } else {
                warn!(
                    "unable to to resolve '~' in path: {}",
                    desktop_file_path_str
                );
                continue;
            }
        }
        let desktop_file_path = desktop_file_path_buf.as_path();
        if !desktop_file_path.exists() {
            warn!(
                "desktop file not found, skipping: {}",
                desktop_file_path_str
            );
            continue;
        }
        let Some(app_info) = DesktopAppInfo::from_filename(desktop_file_path) else {
            warn!(
                "unknown or corrupted desktop file '{:?}'",
                desktop_file_path
            );
            continue;
        };
        res.insert(file.id.clone(), app_info);
    }
    res
}
