use anyhow::Result;
use regex::Regex;
use serde::Deserialize;
use std::{env, fs, io};
use tracing::{error, info};
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
pub struct ApplicationConfig {
    /// used to identify the desktop file in the config
    /// this is either the path or the name
    /// this is for internal use only, not displayed to the user
    #[serde(skip_serializing)]
    #[serde(skip_deserializing)]
    pub id: String,
    /// if set, this name is printed instead of the one in the desktop file
    pub alias: Option<String>,

    /// path or name of the desktop file
    pub desktop_file: Option<String>,
    /// if set, this command is run instead of the desktop file
    /// this is useful for applications that do not have a desktop file
    /// or for custom commands
    pub command: Option<String>,

    /// if set, these prefixes are used to match the URI
    pub prefixes: Option<Vec<String>>,
    /// if set, these regexps are used to match the URI
    pub regexps: Option<Vec<String>>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Config {
    #[serde(rename = "application")]
    pub applications: Vec<ApplicationConfig>,
}

impl Config {
    pub fn read(id: Option<String>) -> Result<Self> {
        let xdg_dirs = BaseDirectories::with_prefix(env!("CARGO_PKG_NAME"));
        let file_path = match id {
            Some(id) => format!("config-{}.toml", id),
            None => "config.toml".into(),
        };
        let config_path = xdg_dirs.place_config_file(file_path)?;
        info!("config path: {}", config_path.display());

        let config_content = fs::read_to_string(&config_path)?;
        let mut config: Config = toml::from_str(&config_content)?;

        for application_config in &mut config.applications {
            // TODO: might compiple regexps here

            if application_config.desktop_file.is_none() && application_config.command.is_none() {
                error!("application config must have either desktop_file or command set");
            }

            if let Some(id) = application_config
                .alias
                .clone()
                .or_else(|| application_config.desktop_file.clone())
            {
                application_config.id = id
            } else {
                error!("application config must have either alias or desktop_file set");
            }
        }

        Ok(config)
    }

    pub fn find_matching_desktop_file(&self, uri: &str) -> Option<&ApplicationConfig> {
        self.applications.iter().find(|df| df.match_uri(uri))
    }
}

impl ApplicationConfig {
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
