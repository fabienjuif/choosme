use std::time::Duration;

use anyhow::Result;
use dbus::blocking::{Connection, Proxy};

pub const DEST: &str = "juif.fabien.choosme";

// dbus-send --print-reply --dest=juif.fabien.choosme / juif.fabien.choosme.Open string:"http://example.com"

pub const OPEN_METHOD: &str = "Open";
pub const OPEN_METHOD_INPUTS: (&str,) = ("uri",);
pub const OPEN_METHOD_OUTPUTS: (&str,) = ("status",);

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
}
