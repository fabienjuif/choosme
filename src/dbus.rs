pub const DEST: &str = "juif.fabien.choosme";

// dbus-send --print-reply --dest=juif.fabien.choosme / juif.fabien.choosme.Toggle

pub const TOGGLE_METHOD: &str = "Toggle";
pub const TOGGLE_METHOD_INPUTS: () = ();
pub const TOGGLE_METHOD_OUTPUTS: (&str,) = ("status",);

#[derive(Debug)]
pub struct ToggleCmdInputs {}

#[derive(Debug)]
pub struct ToggleCmdOutputs {
    pub status: ToggleCmdOutputsStatus,
}

#[derive(Debug, Clone)]
pub enum ToggleCmdOutputsStatus {
    Show,
    Hide,
}

impl From<ToggleCmdOutputsStatus> for String {
    fn from(status: ToggleCmdOutputsStatus) -> Self {
        match status {
            ToggleCmdOutputsStatus::Show => "show".to_string(),
            ToggleCmdOutputsStatus::Hide => "hide".to_string(),
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum ToggleStatusParseError {
    UnknownStatus(String),
    EmptyString,
}

impl TryFrom<String> for ToggleCmdOutputsStatus {
    type Error = ToggleStatusParseError;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        match s.as_str() {
            // Use as_str() to match against string slices
            "show" => Ok(ToggleCmdOutputsStatus::Show),
            "hide" => Ok(ToggleCmdOutputsStatus::Hide),
            _ => {
                if s.is_empty() {
                    return Err(ToggleStatusParseError::EmptyString);
                }
                Err(ToggleStatusParseError::UnknownStatus(s))
            }
        }
    }
}

impl ToggleCmdOutputs {
    pub fn to_dbus_output(&self) -> String {
        self.status.clone().into()
    }
}
