use std::{
    collections::HashMap,
    sync::mpsc::{Receiver, Sender},
    thread::{self, JoinHandle},
    time::Duration,
};

use anyhow::{Context as _, Result, format_err};
use dbus::{MethodErr, blocking::Connection, channel::MatchingReceiver};
use dbus_crossroads::{Context, Crossroads};
use tracing::{debug, info};

use crate::{
    dbus::StatusCmdOutputApplication,
    realm::Realm,
    runner::ApplicationOpenerCommand,
    ui::{OpenWindowParams, UICommand},
};

struct Daemon {
    realms: HashMap<String, Realm>,
    default_applications_ids: HashMap<String, String>,
    application_opener_tx: Sender<ApplicationOpenerCommand>,
    ui_tx: async_channel::Sender<UICommand>,
}

impl Daemon {
    fn open(&self, inputs: crate::dbus::OpenCmdInputs) -> Result<crate::dbus::OpenCmdOutputs> {
        debug!("open command received with inputs: {:?}", inputs);
        let realm = self
            .realms
            .get(&inputs.realm_id)
            .context(format_err!("realm not found: {}", inputs.realm_id))?;

        // try to find a matching desktop file
        if let Some(application) = realm.find_application(&inputs.uri) {
            info!("found matching application: {:?}", application.id);

            // send command to desktop file opener
            self.application_opener_tx
                .send(ApplicationOpenerCommand::Open(crate::runner::OpenParams {
                    realm_id: inputs.realm_id.clone(),
                    uris: vec![inputs.uri],
                    application_id: application.id.clone(),
                }))
                .map_err(|e| anyhow::anyhow!("failed to send command: {}", e))?;

            return Ok(crate::dbus::OpenCmdOutputs {
                status: crate::dbus::OpenCmdOutputsStatus::Launched,
            });
        }

        // fallback to default application if set
        if let Some(default_id) = self.default_applications_ids.get(&inputs.realm_id) {
            if let Some(application) = realm.applications.iter().find(|df| &df.id == default_id) {
                info!("using default application: {:?}", application.id);

                // send command to desktop file opener
                self.application_opener_tx
                    .send(ApplicationOpenerCommand::Open(crate::runner::OpenParams {
                        realm_id: inputs.realm_id.clone(),
                        uris: vec![inputs.uri],
                        application_id: application.id.clone(),
                    }))
                    .map_err(|e| anyhow::anyhow!("failed to send command: {}", e))?;

                return Ok(crate::dbus::OpenCmdOutputs {
                    status: crate::dbus::OpenCmdOutputsStatus::Launched,
                });
            }
        }

        // fallbacking to UI
        info!("no matching desktop file found, falling back to UI");
        self.ui_tx
            .send_blocking(UICommand::OpenWindow(OpenWindowParams {
                realm_id: inputs.realm_id.clone(),
                uris: vec![inputs.uri],
            }))
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

        let realm = self
            .realms
            .get(&inputs.realm_id)
            .context(format_err!("realm not found: {}", inputs.realm_id))?;

        let default_realm_id = self.default_applications_ids.get(&inputs.realm_id);

        Ok(crate::dbus::StatusCmdOutputs {
            applications: realm
                .applications
                .iter()
                .map(|app| StatusCmdOutputApplication {
                    id: app.id.clone(),
                    name: app.display_name.clone(),
                    is_default: default_realm_id == Some(&app.id),
                    icon: app.icon_name.clone().unwrap_or_default(),
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
            self.default_applications_ids.remove(&inputs.realm_id);
            return Ok(crate::dbus::SetDefaultCmdOutputs {});
        }

        let realm = self
            .realms
            .get(&inputs.realm_id)
            .context(format_err!("realm not found: {}", inputs.realm_id))?;
        let application = realm
            .applications
            .get(inputs.index as usize)
            .context(format_err!(
                "application not found at index: {}",
                inputs.index
            ))?;

        self.default_applications_ids
            .insert(inputs.realm_id.clone(), application.id.clone());

        Ok(crate::dbus::SetDefaultCmdOutputs {})
    }
}

pub fn register_dbus(
    application_name: &str,
    realms: HashMap<String, Realm>,
    desktop_files_tx: Sender<ApplicationOpenerCommand>,
    ui_tx: async_channel::Sender<UICommand>,
    shutdown_rx: Receiver<()>,
) -> Result<JoinHandle<()>> {
    debug!("registering dbus for application: {}", application_name);

    // preparing daemon (thread safe is necessary for dbus)
    let daemon = Daemon {
        realms,
        default_applications_ids: HashMap::new(),
        application_opener_tx: desktop_files_tx,
        ui_tx,
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
            move |_: &mut Context, daemon: &mut Daemon, params: (String, String)| {
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
            move |_: &mut Context, daemon: &mut Daemon, params: (String,)| {
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
            move |_: &mut Context, daemon: &mut Daemon, params: (String, i64)| {
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
