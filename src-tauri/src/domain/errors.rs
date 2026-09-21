use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackendError {
    pub code: &'static str,
    pub message: String,
    pub recoverable: bool,
}
pub type Result<T> = std::result::Result<T, BackendError>;
impl BackendError {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            recoverable: true,
        }
    }
    pub fn invalid() -> Self {
        Self::new(
            "SUBSCRIPTION_INVALID",
            "Invalid subscription or node parameters",
        )
    }
    pub fn unsupported() -> Self {
        Self::new(
            "UNSUPPORTED_PROTOCOL",
            "This node configuration cannot be safely converted to Xray",
        )
    }
}
impl std::fmt::Display for BackendError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}
impl std::error::Error for BackendError {}
