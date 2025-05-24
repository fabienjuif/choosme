use adw::prelude::*;
use anyhow::Result;
use gtk4::gio::{self, DesktopAppInfo};
use regex::Regex;
use serde::Deserialize;
use std::path::PathBuf;
use std::{env, fs};
use tracing::{error, info, warn};
use xdg::BaseDirectories;

pub fn read_css_file() -> Result<String> {
    let xdg_dirs = BaseDirectories::with_prefix(env!("CARGO_PKG_NAME"));
    let css_path = xdg_dirs.place_config_file("style.css")?;
    info!("css path: {}", css_path.display());

    let css_content = fs::read_to_string(&css_path)?;

    Ok(css_content)
}

pub fn read_config() -> Result<Config> {
    read_config_file()
        .map_err(|e| anyhow::anyhow!("failed to read config: {}", e))
        .map(Config::new_from_config_file)
}

#[derive(Debug, Deserialize)]
struct DesktopFileConfigFile {
    // TODO: make path optional, and just resolve by name
    path: String,
    /// if set, this name is printed instead of the one in the desktop file
    alias: Option<String>,
    prefixes: Option<Vec<String>>,
    regexps: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct ConfigFile {
    #[serde(rename = "application")]
    desktop_files: Vec<DesktopFileConfigFile>,
}

fn read_config_file() -> Result<ConfigFile> {
    let xdg_dirs = BaseDirectories::with_prefix(env!("CARGO_PKG_NAME"));
    let config_path = xdg_dirs.place_config_file("config.toml")?;
    info!("config path: {}", config_path.display());

    let config_content = fs::read_to_string(&config_path)?;
    let config: ConfigFile = toml::from_str(&config_content)?;

    Ok(config)
}

#[derive(Clone)]
pub struct DesktopFileConfig {
    pub app_info: DesktopAppInfo,
    pub alias: Option<String>,
    pub prefixes: Vec<String>,
    pub regexps: Vec<Regex>,
}

impl DesktopFileConfig {
    pub fn open_on_match(
        &self,
        files: &[gio::File],
        context: Option<&gio::AppLaunchContext>,
    ) -> Result<bool> {
        if self.prefixes.is_empty() && self.regexps.is_empty() {
            return Ok(false);
        }
        let Some(file) = files.first() else {
            return Ok(false);
        };
        // testing prefixes since it should be faster than regexps
        for prefix in &self.prefixes {
            let file_path = file.uri().to_string();
            if file_path.starts_with(prefix) {
                return self.try_open(files, context).map(|_| Ok(true))?;
            }
        }
        // and now regexps
        for regexp in &self.regexps {
            let file_path = file.uri().to_string();
            if regexp.is_match(&file_path) {
                return self.try_open(files, context).map(|_| Ok(true))?;
            }
            info!("regexp: {} - file_path: {}", regexp, file_path);
        }
        Ok(false)
    }

    fn try_open(&self, files: &[gio::File], context: Option<&gio::AppLaunchContext>) -> Result<()> {
        // let app_info = self.app_info.clone();
        self.app_info.launch(files, context)?;
        Ok(())
    }
}

#[derive(Clone)]
pub struct Config {
    pub desktop_files: Vec<DesktopFileConfig>,
}

impl Config {
    fn new_from_config_file(config_file: ConfigFile) -> Self {
        let home_dir_str = env::var("HOME").or_else(|_| env::var("USERPROFILE")).ok();

        let mut desktop_files = Vec::new();
        for file in config_file.desktop_files {
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
            desktop_files.push(DesktopFileConfig {
                app_info,
                alias: file.alias,
                prefixes: file.prefixes.unwrap_or_default(),
                regexps: file
                    .regexps
                    .unwrap_or_default()
                    .iter()
                    .filter_map(|r| match Regex::new(r) {
                        Ok(regex) => Some(regex),
                        Err(e) => {
                            error!("failed to compile regex '{}': {}", r, e);
                            None
                        }
                    })
                    .collect(),
            });
        }
        Config { desktop_files }
    }
}
