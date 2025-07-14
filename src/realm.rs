use std::collections::HashMap;

use crate::{application::Application, config::Config};
use anyhow::Result;
use tracing::{error, warn};

#[derive(Clone, Debug)]
pub struct Realm {
    pub id: String,
    pub applications: Vec<Application>,
}

impl Realm {
    pub fn load_all() -> Result<HashMap<String, Self>> {
        let configs = Config::read_all()?;
        let mut realms = HashMap::new();

        for (id, config) in configs {
            if config.applications.is_empty() {
                warn!("no applications found in realm: {}", id);
                continue;
            }
            let mut applications: Vec<Application> = Vec::with_capacity(config.applications.len());
            for application in &config.applications {
                match Application::new_from_config(application) {
                    Ok(app) => applications.push(app),
                    Err(e) => error!(
                        "failed to create application from config {}: {}: {}",
                        id, application.id, e
                    ),
                }
            }

            realms.insert(id.clone(), Realm { id, applications });
        }
        Ok(realms)
    }

    pub fn find_application(&self, uri: &str) -> Option<&Application> {
        self.applications.iter().find(|df| df.match_uri(uri))
    }
}
