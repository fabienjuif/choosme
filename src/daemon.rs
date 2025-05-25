use std::{
    sync::mpsc::{Receiver, Sender},
    thread::{self, JoinHandle},
    time::Duration,
};

use adw::gio::prelude::{AppInfoExt, IconExt};
use anyhow::Result;
use dbus::{MethodErr, blocking::Connection, channel::MatchingReceiver};
use dbus_crossroads::{Context, Crossroads};
use tracing::{debug, info};

use crate::{
    config::Config,
    dbus::StatusCmdOutputApplication,
    desktop_files::{DesktopFileOpenerCommand, resolve_desktop_files},
};

struct Daemon {
    cfg: Config,
    default_application_id: Option<String>,
    desktop_files_tx: Sender<DesktopFileOpenerCommand>,
    toggle_ui_tx: async_channel::Sender<String>,
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
            .send_blocking(inputs.uri)
            .map_err(|e| anyhow::anyhow!("failed to send toggle UI command: {}", e))?;

        Ok(crate::dbus::OpenCmdOutputs {
            status: crate::dbus::OpenCmdOutputsStatus::Fallbacked,
        })
    }

    fn status(
        &self,
        inputs: crate::dbus::StatusCmdInputs,
    ) -> Result<crate::dbus::StatusCmdOutputs> {
        debug!("status command received with inputs: {:?}", inputs);

        let resolved = resolve_desktop_files(&self.cfg);

        Ok(crate::dbus::StatusCmdOutputs {
            default_application_id: self.default_application_id.clone(),
            applications: self
                .cfg
                .desktop_files
                .iter()
                .map(|df| StatusCmdOutputApplication {
                    id: df.id.clone(),
                    name: df.alias.clone().unwrap_or_else(|| df.id.clone()),
                    icon: resolved
                        .get(&df.id)
                        .and_then(|d| {
                            d.icon()
                                .map(|i| i.to_string().map_or("".to_string(), |i| i.into()))
                        })
                        .unwrap_or("".to_string()),
                })
                .collect(),
        })
    }

    fn kill(&mut self, inputs: crate::dbus::KillCmdInputs) -> Result<crate::dbus::KillCmdOutputs> {
        debug!("kill command received with inputs: {:?}", inputs);

        // TODO: safer way of doing it
        thread::spawn(|| {
            thread::sleep(Duration::from_millis(100));
            std::process::exit(0);
        });

        Ok(crate::dbus::KillCmdOutputs {})
    }

    fn set_default(
        &mut self,
        inputs: crate::dbus::SetDefaultCmdInputs,
    ) -> Result<crate::dbus::SetDefaultCmdOutputs> {
        debug!("set_default command received with inputs: {:?}", inputs);

        if inputs.index < 0 {
            self.default_application_id = None;
            return Ok(crate::dbus::SetDefaultCmdOutputs {});
        }

        let desktop_file = self
            .cfg
            .desktop_files
            .get(inputs.index as usize)
            .ok_or_else(|| anyhow::anyhow!("invalid index: {}", inputs.index))?;

        self.default_application_id = Some(desktop_file.id.clone());

        Ok(crate::dbus::SetDefaultCmdOutputs {})
    }
}

pub fn register_dbus(
    application_name: &str,
    cfg: Config,
    desktop_files_tx: Sender<DesktopFileOpenerCommand>,
    toggle_ui_tx: async_channel::Sender<String>,
    shutdown_rx: Receiver<()>,
) -> Result<JoinHandle<()>> {
    debug!("registering dbus for application: {}", application_name);

    // preparing daemon (thread safe is necessary for dbus)
    // TODO:
    let daemon = Daemon {
        cfg,
        default_application_id: None,
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

        b.method(
            crate::dbus::STATUS_METHOD,
            crate::dbus::STATUS_METHOD_INPUTS,
            crate::dbus::STATUS_METHOD_OUTPUTS,
            move |_: &mut Context, daemon: &mut Daemon, params: ()| {
                let inputs = crate::dbus::StatusCmdInputs::from_dbus_input(params);
                let output = daemon
                    .status(inputs)
                    .map_err(|e| MethodErr::failed(&e.to_string()))?
                    .to_dbus_output();
                Ok(output)
            },
        );

        b.method(
            crate::dbus::KILL_METHOD,
            crate::dbus::KILL_METHOD_INPUTS,
            crate::dbus::KILL_METHOD_OUTPUTS,
            move |_: &mut Context, daemon: &mut Daemon, params: ()| {
                let inputs = crate::dbus::KillCmdInputs::from_dbus_input(params);
                daemon
                    .kill(inputs)
                    .map_err(|e| MethodErr::failed(&e.to_string()))?
                    .to_dbus_output();
                Ok(())
            },
        );

        b.method(
            crate::dbus::SET_DEFAULT_METHOD,
            crate::dbus::SET_DEFAULT_METHOD_INPUTS,
            crate::dbus::SET_DEFAULT_METHOD_OUTPUTS,
            move |_: &mut Context, daemon: &mut Daemon, params: (i64,)| {
                let inputs = crate::dbus::SetDefaultCmdInputs::from_dbus_input(params);
                daemon
                    .set_default(inputs)
                    .map_err(|e| MethodErr::failed(&e.to_string()))?
                    .to_dbus_output();
                Ok(())
            },
        );
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
