use anyhow::Result;
use regex::Regex;
use serde::Deserialize;
use std::{env, fs, io};
use tracing::info;
use xdg::BaseDirectories;

pub fn read_css_file() -> Result<String> {
    let xdg_dirs = BaseDirectories::with_prefix(env!("CARGO_PKG_NAME"));
    let css_path = xdg_dirs.place_config_file("style.css")?;
    info!("css path: {}", css_path.display());

    match fs::read_to_string(&css_path) {
        Ok(css_content) => Ok(css_content),
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            info!("css file not found, using default style");
            let content = include_str!("../style.css").to_string();
            fs::write(&css_path, &content)
                .map_err(|e| e.into())
                .map(|_| content)
        }
        Err(e) => Err(e.into()),
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct DesktopFileConfig {
    /// used to identify the desktop file in the config
    /// this is either the path or the name
    /// this is for internal use only, not displayed to the user
    #[serde(skip_serializing)]
    #[serde(skip_deserializing)]
    pub id: String,
    // TODO: make path optional, and just resolve by name
    pub path: String,
    /// if set, this name is printed instead of the one in the desktop file
    pub alias: Option<String>,
    pub prefixes: Option<Vec<String>>,
    pub regexps: Option<Vec<String>>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Config {
    #[serde(rename = "application")]
    pub desktop_files: Vec<DesktopFileConfig>,
}

impl Config {
    pub fn read() -> Result<Self> {
        let xdg_dirs = BaseDirectories::with_prefix(env!("CARGO_PKG_NAME"));
        let config_path = xdg_dirs.place_config_file("config.toml")?;
        info!("config path: {}", config_path.display());

        let config_content = fs::read_to_string(&config_path)?;
        let mut config: Config = toml::from_str(&config_content)?;

        for desktop_file in &mut config.desktop_files {
            // TODO: might compiple regexps here

            desktop_file.id = desktop_file.path.clone();
        }

        Ok(config)
    }

    pub fn find_matching_desktop_file(&self, uri: &str) -> Option<&DesktopFileConfig> {
        self.desktop_files.iter().find(|df| df.match_uri(uri))
    }
}

impl DesktopFileConfig {
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
