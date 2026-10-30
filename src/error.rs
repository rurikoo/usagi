use std::{error::Error as StdError, ffi::CStr, fmt};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    operation: &'static str,
    message: String,
}

impl Error {
    pub(crate) fn from_native(operation: &'static str, fallback: &str) -> Self {
        let ptr = unsafe { crate::ffi::usagi_last_error() };

        let message = if ptr.is_null() {
            String::new()
        } else {
            unsafe { CStr::from_ptr(ptr) }
                .to_string_lossy()
                .into_owned()
        };

        Self {
            operation,
            message: if message.is_empty() {
                fallback.to_owned()
            } else {
                message
            },
        }
    }

    pub(crate) fn new(operation: &'static str, message: impl Into<String>) -> Self {
        Self {
            operation,
            message: message.into(),
        }
    }

    pub fn operation(&self) -> &'static str {
        self.operation
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.operation, self.message)
    }
}

impl StdError for Error {}
