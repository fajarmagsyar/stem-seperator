use crate::types::ContractError;
use std::{error::Error, fmt, io};

#[derive(Debug)]
pub enum StemError {
    Contract(ContractError),
    Io(io::Error),
    Inference(String),
    Unavailable(String),
    Decode(String),
    InvalidPackage(String),
    Export(String),
    Cancelled,
}

impl fmt::Display for StemError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Contract(error) => error.fmt(f),
            Self::Io(error) => error.fmt(f),
            Self::Inference(message)
            | Self::Unavailable(message)
            | Self::Decode(message)
            | Self::InvalidPackage(message)
            | Self::Export(message) => f.write_str(message),
            Self::Cancelled => f.write_str("Processing cancelled"),
        }
    }
}

impl Error for StemError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Contract(error) => Some(error),
            Self::Io(error) => Some(error),
            _ => None,
        }
    }
}

impl From<ContractError> for StemError {
    fn from(error: ContractError) -> Self {
        Self::Contract(error)
    }
}

impl From<io::Error> for StemError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}
