use anyhow::Result;
use dbus::{MethodErr, blocking::Connection};
use dbus_crossroads::{Context, Crossroads};

use crate::config::Config;

#[derive(Default)]
struct Daemon {}

impl Daemon {
    fn toggle(&self) -> Result<crate::dbus::ToggleCmdOutputs> {
        // TODO:

        Ok(crate::dbus::ToggleCmdOutputs {
            status: crate::dbus::ToggleCmdOutputsStatus::Show,
        })
    }
}

pub fn register_dbus(application_id: &str, application_name: &str, cfg: &Config) -> Result<()> {
    // preparing daemon (thread safe is necessary for dbus)
    // TODO:
    let daemon = Daemon::default();

    // dbus descriptions
    let c = Connection::new_session()?;
    c.request_name(crate::dbus::DEST, false, true, false)?;
    let mut cr = Crossroads::new();
    let iface_token = cr.register(crate::dbus::DEST, |b| {
        b.method(
            crate::dbus::TOGGLE_METHOD,
            crate::dbus::TOGGLE_METHOD_INPUTS,
            crate::dbus::TOGGLE_METHOD_OUTPUTS,
            move |_: &mut Context, daemon: &mut Daemon, _: ()| {
                let output = daemon
                    .toggle()
                    .map_err(|e| MethodErr::failed(&e.to_string()))?
                    .to_dbus_output();
                Ok((output,))
            },
        );

        // TODO: other methods
    });

    // starting dbus server
    cr.insert("/", &[iface_token], daemon);
    cr.serve(&c)?;
    unreachable!()
}
