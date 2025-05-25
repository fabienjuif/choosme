use std::{
    sync::mpsc::{Receiver, Sender},
    thread::JoinHandle,
};

use anyhow::Result;
use dbus::{MethodErr, blocking::Connection, channel::MatchingReceiver};
use dbus_crossroads::{Context, Crossroads};
use tracing::{debug, info};

use crate::{config::Config, desktop_files::DesktopFileOpenerCommand};

struct Daemon {
    cfg: Config,
    desktop_files_tx: Sender<DesktopFileOpenerCommand>,
    toggle_ui_tx: Sender<String>,
}

impl Daemon {
    fn open(&self, inputs: crate::dbus::OpenCmdInputs) -> Result<crate::dbus::OpenCmdOutputs> {
        debug!("open command received with inputs: {:?}", inputs);

        if let Some(desktop_file) = self.cfg.find_matching_desktop_file(&inputs.uri) {
            info!("found matching desktop file: {:?}", desktop_file.id);

            // send command to desktop file opener
            self.desktop_files_tx
                .send(DesktopFileOpenerCommand::Open(
                    crate::desktop_files::OpenParams {
                        uris: vec![inputs.uri],
                        desktop_file_id: desktop_file.id.clone(),
                    },
                ))
                .map_err(|e| anyhow::anyhow!("failed to send command: {}", e))?;

            return Ok(crate::dbus::OpenCmdOutputs {
                status: crate::dbus::OpenCmdOutputsStatus::Launched,
            });
        }

        // fallbacking to UI
        info!("no matching desktop file found, falling back to UI");
        self.toggle_ui_tx
            .send(inputs.uri)
            .map_err(|e| anyhow::anyhow!("failed to send toggle UI command: {}", e))?;

        Ok(crate::dbus::OpenCmdOutputs {
            status: crate::dbus::OpenCmdOutputsStatus::Fallbacked,
        })
    }
}

pub fn register_dbus(
    application_name: &str,
    cfg: Config,
    desktop_files_tx: Sender<DesktopFileOpenerCommand>,
    toggle_ui_tx: Sender<String>,
    shutdown_rx: Receiver<()>,
) -> Result<JoinHandle<()>> {
    debug!("registering dbus for application: {}", application_name);

    // preparing daemon (thread safe is necessary for dbus)
    // TODO:
    let daemon = Daemon {
        cfg,
        desktop_files_tx,
        toggle_ui_tx,
    };

    // dbus descriptions
    let c = Connection::new_session()?;
    c.request_name(crate::dbus::DEST, false, true, false)?;
    let mut cr = Crossroads::new();
    let iface_token = cr.register(crate::dbus::DEST, |b| {
        b.method(
            crate::dbus::OPEN_METHOD,
            crate::dbus::OPEN_METHOD_INPUTS,
            crate::dbus::OPEN_METHOD_OUTPUTS,
            move |_: &mut Context, daemon: &mut Daemon, params: (String,)| {
                let inputs = crate::dbus::OpenCmdInputs::from_dbus_input(params);
                let output = daemon
                    .open(inputs)
                    .map_err(|e| MethodErr::failed(&e.to_string()))?
                    .to_dbus_output();
                Ok(output)
            },
        );

        // TODO: other methods
    });
    cr.insert("/", &[iface_token], daemon);

    // starting dbus server
    let jh = std::thread::spawn(move || {
        c.start_receive(
            dbus::message::MatchRule::new_method_call(),
            Box::new(move |msg, conn| {
                cr.handle_message(msg, conn).unwrap();
                true
            }),
        );

        // loop while not shutdown
        loop {
            match shutdown_rx.try_recv() {
                Ok(_) => {
                    break;
                }
                Err(e) => {
                    match e {
                        std::sync::mpsc::TryRecvError::Disconnected => {
                            break;
                        }
                        std::sync::mpsc::TryRecvError::Empty => {
                            // No shutdown signal received, continue processing
                        }
                    }
                }
            }

            let _ = c.process(std::time::Duration::from_millis(1000));
        }

        info!("D-Bus thread exiting");
    });

    Ok(jh)
}
