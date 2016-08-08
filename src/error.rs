
use std::error;
use std::fmt;
use std::io;


/// PRU subsystem error.
#[derive(Clone, Debug, PartialEq)]
pub enum Error {
    AlreadyInstantiated,
    PermissionDenied,
    DeviceNotFound,
    OtherDeviceError
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "PRU error")
    }
}

impl error::Error for Error {
    fn description(&self) -> &str {
        match *self {
            Error::AlreadyInstantiated => "already instantiated",
            Error::PermissionDenied => "permission denied",
            Error::DeviceNotFound => "device not found",
            Error::OtherDeviceError => "other device error",
        }
    }
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Error {
        match err.kind() {
            io::ErrorKind::NotFound => Error::DeviceNotFound,
            io::ErrorKind::PermissionDenied => Error::PermissionDenied,
            _ => Error::OtherDeviceError
        }
    }
}
