use thiserror::Error;

/// Failure reported by a bulb in a RESULT message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceError {
    pub code: i32,
    pub message: String,
}

impl DeviceError {
    pub fn is_quota(&self) -> bool {
        self.message.to_ascii_lowercase().contains("quota")
    }
}

impl std::fmt::Display for DeviceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.message, self.code)
    }
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("{0}")]
    Parse(String),
    #[error("{0}")]
    Rejected(&'static str),
    #[error("device error: {0}")]
    Device(DeviceError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("timed out")]
    Timeout,
    #[error("connection closed")]
    Closed,
    #[error("not supported by this light")]
    Unsupported,
}
