// --------------------------------------------------------
// Module that handles launching applications based on desktop files or commands.
// --------------------------------------------------------

use gdk4::gio::AppLaunchContext;
use std::{
    sync::mpsc::{self, Sender},
    thread::JoinHandle,
};
use tracing::{error, info};

use crate::realm::{self};

#[derive(Debug)]
pub struct OpenParams {
    pub uris: Vec<String>,

    /// The ID of the application to launch.
    /// It has to be resolved beforce sending the Launch command. (Via UI for example).
    pub application_id: String,

    /// The ID of the realm to use for launching the application.
    pub realm_id: String,
}

pub enum ApplicationOpenerCommand {
    /// Open an application by its ID.
    Open(OpenParams),

    /// Quit.
    Quit,
}

/// Starts the application opener thread.
pub fn start_applications_opener() -> (JoinHandle<()>, Sender<ApplicationOpenerCommand>) {
    let (tx, rx) = mpsc::channel();

    let jh = std::thread::spawn(move || {
        // TODO: avoid reloading realms here, for now we are doing this because
        //     : we can not pass realms between threads because of gio *void
        let realms = match realm::Realm::load_all() {
            Ok(realms) => realms,
            Err(e) => {
                error!("failed to load realms: {}", e);
                return;
            }
        };

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

                    let realm = match realms.get(&params.realm_id) {
                        Some(realm) => realm,
                        None => {
                            error!("no realm found for id: {}", params.realm_id);
                            continue;
                        }
                    };

                    let uris = params
                        .uris
                        .iter()
                        .map(|s| s.as_str())
                        .collect::<Vec<&str>>();

                    let Some(application) = realm
                        .applications
                        .iter()
                        .find(|app| app.id == params.application_id)
                    else {
                        error!("no application found for id: {}", params.application_id);
                        continue;
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
