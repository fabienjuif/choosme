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
}

#[derive(Debug)]
pub struct OpenCmdOutputs {
    pub status: OpenCmdOutputsStatus,
}

impl OpenCmdOutputs {
    pub fn to_dbus_output(&self) -> (String,) {
        (self.status.clone().into(),)
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
