use std::time::Duration;

use anyhow::Result;
use dbus::blocking::{Connection, Proxy};
use serde::Serialize;

pub const DEST: &str = "juif.fabien.choosme";

// dbus-send --print-reply --dest=juif.fabien.choosme / juif.fabien.choosme.Open string:"http://example.com"

pub const OPEN_METHOD: &str = "Open";
pub const OPEN_METHOD_INPUTS: (&str,) = ("uri",);
pub const OPEN_METHOD_OUTPUTS: (&str,) = ("status",);

// dbus-send --print-reply --dest=juif.fabien.choosme / juif.fabien.choosme.Status

pub const STATUS_METHOD: &str = "Status";
pub const STATUS_METHOD_INPUTS: () = ();
pub const STATUS_METHOD_OUTPUTS: (&str, &str) = ("applications_ids", "default_application");

// dbus-send --print-reply --dest=juif.fabien.choosme / juif.fabien.choosme.Kill

pub const KILL_METHOD: &str = "Kill";
pub const KILL_METHOD_INPUTS: () = ();
pub const KILL_METHOD_OUTPUTS: () = ();

// dbus-send --print-reply --dest=juif.fabien.choosme / juif.fabien.choosme.SetDefault int64:1

pub const SET_DEFAULT_METHOD: &str = "SetDefault";
pub const SET_DEFAULT_METHOD_INPUTS: (&str,) = ("index",);
pub const SET_DEFAULT_METHOD_OUTPUTS: () = ();

#[derive(Debug)]
pub struct OpenCmdInputs {
    pub uri: String,
}

impl OpenCmdInputs {
    pub fn from_dbus_input(input: (String,)) -> Self {
        OpenCmdInputs { uri: input.0 }
    }

    pub fn to_dbus_input(&self) -> (String,) {
        (self.uri.clone(),)
    }
}

#[derive(Debug)]
pub struct OpenCmdOutputs {
    pub status: OpenCmdOutputsStatus,
}

impl OpenCmdOutputs {
    pub fn to_dbus_output(&self) -> (String,) {
        (self.status.clone().into(),)
    }

    pub fn from_dbus_output(output: (String,)) -> Result<Self, ToggleStatusParseError> {
        let status = OpenCmdOutputsStatus::try_from(output.0)?;
        Ok(OpenCmdOutputs { status })
    }
}

#[derive(Debug, Clone)]
pub enum OpenCmdOutputsStatus {
    /// No application was launched, the UI was used instead.
    Fallbacked,
    /// An application was launched.
    Launched,
}

impl From<OpenCmdOutputsStatus> for String {
    fn from(status: OpenCmdOutputsStatus) -> Self {
        match status {
            OpenCmdOutputsStatus::Fallbacked => "fallbacked".to_string(),
            OpenCmdOutputsStatus::Launched => "launched".to_string(),
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum ToggleStatusParseError {
    UnknownStatus(String),
    EmptyString,
}

impl std::fmt::Display for ToggleStatusParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ToggleStatusParseError::UnknownStatus(s) => write!(f, "Unknown status: {}", s),
            ToggleStatusParseError::EmptyString => write!(f, "Empty string provided"),
        }
    }
}

impl TryFrom<String> for OpenCmdOutputsStatus {
    type Error = ToggleStatusParseError;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        match s.as_str() {
            "fallbacked" => Ok(OpenCmdOutputsStatus::Fallbacked),
            "launched" => Ok(OpenCmdOutputsStatus::Launched),
            _ => {
                if s.is_empty() {
                    return Err(ToggleStatusParseError::EmptyString);
                }
                Err(ToggleStatusParseError::UnknownStatus(s))
            }
        }
    }
}

#[derive(Debug)]
pub struct StatusCmdInputs {}

impl StatusCmdInputs {
    pub fn from_dbus_input(_input: ()) -> Self {
        StatusCmdInputs {}
    }

    pub fn to_dbus_input(&self) {}
}

#[derive(Debug, Serialize)]
pub struct StatusCmdOutputs {
    pub applications: Vec<StatusCmdOutputApplication>,
    pub default_application_id: Option<String>,
}

impl StatusCmdOutputs {
    pub fn to_dbus_output(&self) -> (Vec<(String, String, String)>, String) {
        (
            self.applications
                .iter()
                .map(|app| (app.id.clone(), app.name.clone(), app.icon.clone()))
                .collect(),
            self.default_application_id.clone().unwrap_or_default(),
        )
    }

    pub fn from_dbus_output(output: (Vec<(String, String, String)>, String)) -> Result<Self, ()> {
        Ok(StatusCmdOutputs {
            applications: output
                .0
                .into_iter()
                .map(|(id, name, icon)| StatusCmdOutputApplication { id, name, icon })
                .collect(),
            default_application_id: if output.1.is_empty() {
                None
            } else {
                Some(output.1)
            },
        })
    }
}

#[derive(Debug)]
pub struct KillCmdInputs {}

impl KillCmdInputs {
    pub fn from_dbus_input(_input: ()) -> Self {
        Self {}
    }

    #[allow(clippy::unused_unit)]
    pub fn to_dbus_input(&self) -> () {
        ()
    }
}

#[derive(Debug)]
pub struct KillCmdOutputs {}

impl KillCmdOutputs {
    #[allow(clippy::unused_unit)]
    pub fn to_dbus_output(&self) -> () {
        ()
    }

    pub fn from_dbus_output(_output: ()) -> Result<Self> {
        Ok(Self {})
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct StatusCmdOutputApplication {
    pub id: String,
    pub name: String,
    pub icon: String,
}

#[derive(Debug)]
pub struct SetDefaultCmdInputs {
    pub index: i64,
}

impl SetDefaultCmdInputs {
    pub fn from_dbus_input(input: (i64,)) -> Self {
        Self { index: input.0 }
    }

    #[allow(clippy::unused_unit)]
    pub fn to_dbus_input(&self) -> (i64,) {
        (self.index,)
    }
}

#[derive(Debug)]
pub struct SetDefaultCmdOutputs {}

impl SetDefaultCmdOutputs {
    #[allow(clippy::unused_unit)]
    pub fn to_dbus_output(&self) -> () {
        ()
    }

    pub fn from_dbus_output(_output: ()) -> Result<Self> {
        Ok(Self {})
    }
}

pub struct DBUSClient {
    // We remove the proxy from the struct because it borrows from the connection.
    // Instead, we'll create proxies on demand or pass the connection around.
    connection: Connection,
}

impl DBUSClient {
    pub fn new() -> Result<Self, dbus::Error> {
        let c = Connection::new_session()?;
        Ok(DBUSClient { connection: c })
    }

    fn get_proxy(&self) -> Proxy<'_, &Connection> {
        self.connection
            .with_proxy(DEST, "/", Duration::from_millis(2000))
    }

    pub fn open(&self, uri: &str) -> Result<OpenCmdOutputs> {
        let msg = OpenCmdInputs {
            uri: uri.to_string(),
        };
        let result = self
            .get_proxy()
            .method_call(DEST, OPEN_METHOD, msg.to_dbus_input())?;
        let out = OpenCmdOutputs::from_dbus_output(result)
            .map_err(|e| dbus::Error::new_failed(&e.to_string()))?;

        Ok(out)
    }

    pub fn status(&self) -> Result<StatusCmdOutputs> {
        let msg = StatusCmdInputs {};
        let result = self
            .get_proxy()
            .method_call(DEST, STATUS_METHOD, msg.to_dbus_input())?;
        let out = StatusCmdOutputs::from_dbus_output(result)
            .expect("StatusCmdOutputs::from_dbus_output should not fail");
        Ok(out)
    }

    pub fn kill(&self) -> Result<KillCmdOutputs> {
        let msg = KillCmdInputs {};
        #[allow(clippy::let_unit_value)]
        let result = self
            .get_proxy()
            .method_call(DEST, KILL_METHOD, msg.to_dbus_input())?;
        let out = KillCmdOutputs::from_dbus_output(result)
            .map_err(|e| dbus::Error::new_failed(&e.to_string()))?;
        Ok(out)
    }

    pub fn set_default(&self, index: i64) -> Result<SetDefaultCmdOutputs> {
        let msg = SetDefaultCmdInputs { index };
        #[allow(clippy::let_unit_value)]
        let result = self
            .get_proxy()
            .method_call(DEST, SET_DEFAULT_METHOD, msg.to_dbus_input())?;
        let out = SetDefaultCmdOutputs::from_dbus_output(result)
            .map_err(|e| dbus::Error::new_failed(&e.to_string()))?;
        Ok(out)
    }
}
