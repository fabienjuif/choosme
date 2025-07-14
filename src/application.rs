use anyhow::Result;
use gdk4::gio::{self, AppLaunchContext};
use gtk4::gio::{DesktopAppInfo, prelude::AppInfoExt};
use regex::Regex;
use std::{
    collections::HashMap,
    env,
    path::PathBuf,
    sync::{mpsc::{self, Sender}, Arc},
    thread::JoinHandle,
};
use tracing::{debug, error, info, warn};

use crate::config::Config;

// TODO: make a read only singleton shared memory?
//     : for now this is not possible because of gio *void
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

/// Represents an application that can be launched.
/// It can either be a desktop application with a `.desktop` file or a command that can be run.
#[derive(Clone, Debug)]
pub struct Application {
    pub id: String,
    pub display_name: String,
    pub icon: Option<gio::Icon>,
    prefixes: Option<Vec<String>>,
    regexps: Option<Vec<String>>,

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
            prefixes: application_config.prefixes.clone(),
            regexps: application_config.regexps.clone(),
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

    pub fn match_uri(&self, uri: &str) -> bool {
        if self.prefixes.is_none() && self.regexps.is_none() {
            return false;
        }
        // testing prefixes since it should be faster than regexps
        if let Some(prefixes) = &self.prefixes {
            for prefix in prefixes {
                if uri.starts_with(prefix) {
                    return true;
                }
            }
        }
        // and now regexps
        if let Some(regexps) = &self.regexps {
            for regexp in regexps {
                // TODO: maybe cache regexps later
                if Regex::new(regexp).map(|r| r.is_match(uri)).unwrap_or(false) {
                    return true;
                }
            }
        }
        false
    }
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
