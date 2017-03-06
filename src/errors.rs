// Copyright 2016 Google Inc. All Rights Reserved.
//
// Licensed under the MIT License, <LICENSE or http://opensource.org/licenses/MIT>.
// This file may not be copied, modified, or distributed except according to those terms.

use serde::{Deserialize, Serialize};
use std::{fmt, io};
use std::error::Error as StdError;

/// All errors that can occur during the use of tarpc.
#[derive(Debug)]
pub enum Error<E> {
    /// Any IO error.
    Io(io::Error),
    /// The server was unable to reply to the rpc for some reason.
    ///
    /// This is a service-specific error. Its type is individually specified in the
    /// `service!` macro for each rpc.
    App(E),
}

impl<E: StdError + Deserialize + Serialize + Send + 'static> fmt::Display for Error<E> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Error::App(ref e) => fmt::Display::fmt(e, f),
            Error::Io(ref e) => fmt::Display::fmt(e, f),
        }
    }
}

impl<E: StdError + Deserialize + Serialize + Send + 'static> StdError for Error<E> {
    fn description(&self) -> &str {
        match *self {
            Error::App(ref e) => e.description(),
            Error::Io(ref e) => e.description(),
        }
    }

    fn cause(&self) -> Option<&StdError> {
        match *self {
            Error::App(_) => None,
            Error::Io(ref e) => e.cause(),
        }
    }
}

impl<E> From<io::Error> for Error<E> {
    fn from(err: io::Error) -> Self {
        Error::Io(err)
    }
}

/// Convert `native_tls::Error` to `std::io::Error`
#[cfg(feature = "tls")]
pub fn native_to_io(e: ::native_tls::Error) -> io::Error {
    io::Error::new(io::ErrorKind::Other, e)
}
