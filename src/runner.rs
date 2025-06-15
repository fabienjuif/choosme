// --------------------------------------------------------
// Module that handles launching applications based on desktop files or commands.
// --------------------------------------------------------

use anyhow::Result;
use gdk4::gio::{self, AppLaunchContext};
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

    /// The ID of the application to launch.
    /// It has to be resolved beforce sending the Launch command. (Via UI for example).
    pub application_id: String,
}

pub enum ApplicationOpenerCommand {
    /// Open an application by its ID.
    Open(OpenParams),

    /// Quit.
    Quit,
}

/// Starts the application opener thread.
/// It resolves the desktop files from the config and listens for commands to open applications.
/// It returns a JoinHandle to the thread and a Sender to send commands to the thread.
/// The thread will run until it receives a Quit command.
/// The commands are sent via the `Sender<ApplicationOpenerCommand>`.
pub fn start_applications_opener(
    cfg: Config,
) -> (JoinHandle<()>, Sender<ApplicationOpenerCommand>) {
    let (tx, rx) = mpsc::channel();

    let jh = std::thread::spawn(move || {
        let applications_by_id = from_config(&cfg);

        loop {
            match rx.recv() {
                Ok(ApplicationOpenerCommand::Quit) => {
                    info!("received command to quit application opener");
                    break;
                }
                Ok(ApplicationOpenerCommand::Open(params)) => {
                    info!(
                        "received command to open application with params: {:?}",
                        params
                    );

                    let uris = params
                        .uris
                        .iter()
                        .map(|s| s.as_str())
                        .collect::<Vec<&str>>();

                    let Some(application) = applications_by_id.get(&params.application_id) else {
                        error!("no application found for id: {}", params.application_id);
                        return;
                    };
                    if let Err(e) = application.run(&uris, None::<&AppLaunchContext>) {
                        error!(
                            "failed to open desktop file '{}': {}",
                            params.application_id, e
                        );
                    }
                }
                Err(_) => {
                    error!("error receiving command from run_application_opener channel");
                    break;
                }
            }
        }
    });

    (jh, tx)
}

/// Represents an application that can be launched.
/// It can either be a desktop application with a `.desktop` file or a command that can be run.
pub struct Application {
    pub id: String,
    pub display_name: String,
    pub icon: Option<gio::Icon>,

    desktop_app_info: Option<DesktopAppInfo>,
    command: Option<String>,
}

impl Application {
    pub fn new_from_config(application_config: &crate::config::ApplicationConfig) -> Result<Self> {
        let desktop_app_info = resolve_desktop_file_from_config(application_config);

        Ok(Application {
            id: application_config.id.clone(),
            desktop_app_info: desktop_app_info.clone(),
            command: application_config.command.clone(),
            display_name: application_config
                .alias
                .clone()
                .or_else(|| desktop_app_info.as_ref().map(|d| d.name().into()))
                .unwrap_or_else(|| application_config.id.clone()),
            icon: desktop_app_info.and_then(|d| d.icon()),
        })
    }

    /// Runs the application with the given URIs.
    /// If the application is a desktop application, it will launch it with the URIs.
    /// If the application is a command, it will run the command with the URIs as arguments.
    /// If the application is neither, it will return an error.
    pub fn run(&self, uris: &[&str], context: Option<&AppLaunchContext>) -> Result<()> {
        if let Some(app_info) = &self.desktop_app_info {
            debug!("launching application '{}' with URIs: {:?}", self.id, uris);
            app_info.launch_uris(uris, context)?;
            return Ok(());
        }

        if let Some(command) = &self.command {
            debug!(
                "running command '{}' for application '{}' with URIs: {:?}",
                command, self.id, uris
            );
            let command_str = command.replace("%u", &uris.join(" "));
            // we spawn and forget
            // TODO: maybe spawn in a new thread + log?
            std::process::Command::new("sh")
                .args(vec!["-c", &command_str])
                .spawn()?;
            return Ok(());
        }

        Err(anyhow::anyhow!(
            "no desktop app info or command to run for application '{}'",
            self.id
        ))
    }
}

#[deprecated]
pub fn resolve_desktop_files(config_file: &Config) -> HashMap<String, DesktopAppInfo> {
    let mut res = HashMap::new();
    for file in config_file.applications.iter() {
        if let Some(app_info) = resolve_desktop_file_from_config(file) {
            res.insert(file.id.clone(), app_info);
        }
    }
    res
}

// TODO: make a read only singleton shared memory?
pub fn from_config(config: &crate::config::Config) -> HashMap<String, Application> {
    let mut res = HashMap::new();
    for application_config in &config.applications {
        match Application::new_from_config(application_config) {
            Ok(application) => {
                res.insert(application.id.clone(), application);
            }
            Err(e) => {
                error!(
                    "failed to create application from config: {}: {}",
                    application_config.id, e
                );
            }
        }
    }
    res
}

fn resolve_desktop_file_from_config(
    application_config: &crate::config::ApplicationConfig,
) -> Option<DesktopAppInfo> {
    let Some(desktop_file_path_str) = &application_config.desktop_file else {
        return None;
    };
    if desktop_file_path_str.is_empty() {
        warn!("desktop file path is empty, skipping");
        return None;
    }

    let home_dir_str = env::var("HOME").or_else(|_| env::var("USERPROFILE")).ok();

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
            return None;
        }
    }
    let desktop_file_path = desktop_file_path_buf.as_path();
    if !desktop_file_path.exists() {
        warn!(
            "desktop file not found, skipping: {}",
            desktop_file_path_str
        );
        return None;
    }
    DesktopAppInfo::from_filename(desktop_file_path)
}
