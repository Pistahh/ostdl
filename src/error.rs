use std::borrow::Cow;
use std::borrow::Cow::Borrowed;
use std::io;

use xmlrpc::{Error as RequestError, Fault};

/// A commonly used Error
pub(crate) const E_INV_RESP: Error = Error::Ost(Borrowed("invalid xml-rpc response"));

/// All the errors that can occur
#[derive(Debug)]
pub(crate) enum Error {
    Io(io::Error),
    Ost(Cow<'static, str>),
    XmlRpcRequest(RequestError),
    XmlRpcFault(Fault),
    Reqwest(reqwest::Error),
}

/// Converting all sub-errors into Error.

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Error {
        Error::Io(e)
    }
}

impl From<&'static str> for Error {
    fn from(e: &'static str) -> Error {
        Error::Ost(e.into())
    }
}

impl From<RequestError> for Error {
    fn from(e: RequestError) -> Error {
        Error::XmlRpcRequest(e)
    }
}

impl From<Fault> for Error {
    fn from(e: Fault) -> Error {
        Error::XmlRpcFault(e)
    }
}

impl From<reqwest::Error> for Error {
    fn from(e: reqwest::Error) -> Error {
        Error::Reqwest(e)
    }
}

/// Prints an error to stderr
pub(crate) fn print_err(err: String) {
    eprintln!("{}", err);
}

/// If the input is an Error then prints it to stderr
pub(crate) fn print_if_err<T>(res: &Result<T, Error>) {
    if let Err(ref err) = res {
        match err {
            Error::Ost(ref e) => eprintln!("{}", e.to_string()),
            Error::Io(ref e) => eprintln!("{}", e.to_string()),
            Error::XmlRpcRequest(ref e) => eprintln!("{}", e.to_string()),
            Error::XmlRpcFault(ref e) => eprintln!("{}", e.to_string()),
            Error::Reqwest(ref e) => eprintln!("{}", e.to_string()),
        }
    }
}
