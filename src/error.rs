use std::array::TryFromSliceError;

pub type Result<T> = core::result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    General(String),
    FrameIncomplete,
    Io(std::io::Error),
    Serde(serde_json::Error),
    InvalidBinaryFormat(TryFromSliceError),
    InvalidProtocol(String),
}

impl core::fmt::Display for Error {
    fn fmt(&self, fmt: &mut core::fmt::Formatter) -> core::result::Result<(), core::fmt::Error> {
        write!(fmt, "{self:?}")
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<serde_json::Error> for Error {
    fn from(value: serde_json::Error) -> Self {
        Self::Serde(value)
    }
}

// And implement From<TryFromSliceError>
impl From<TryFromSliceError> for Error {
    fn from(value: TryFromSliceError) -> Self {
        Self::InvalidBinaryFormat(value)
    }
}
