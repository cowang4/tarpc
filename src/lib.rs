// Copyright 2016 Google Inc. All Rights Reserved.
//
// Licensed under the MIT License, <LICENSE or http://opensource.org/licenses/MIT>.
// This file may not be copied, modified, or distributed except according to those terms.

//! tarpc is an RPC framework for rust with a focus on ease of use. Defining a
//! service can be done in just a few lines of code, and most of the boilerplate of
//! writing a server is taken care of for you.
//!
//! ## What is an RPC framework?
//! "RPC" stands for "Remote Procedure Call," a function call where the work of
//! producing the return value is being done somewhere else. When an rpc function is
//! invoked, behind the scenes the function contacts some other process somewhere
//! and asks them to evaluate the function instead. The original function then
//! returns the value produced by the other process.
//!
//! RPC frameworks are a fundamental building block of most microservices-oriented
//! architectures. Two well-known ones are [gRPC](http://www.grpc.io) and
//! [Cap'n Proto](https://capnproto.org/).
//!
//! tarpc differentiates itself from other RPC frameworks by defining the schema in code,
//! rather than in a separate language such as .proto. This means there's no separate compilation
//! process, and no cognitive context switching between different languages. Additionally, it
//! works with the community-backed library serde: any serde-serializable type can be used as
//! arguments to tarpc fns.

#![deny(missing_docs, missing_debug_implementations)]
#![feature(never_type)]
#![cfg_attr(test, feature(plugin))]
#![cfg_attr(test, plugin(tarpc_plugins))]

extern crate byteorder;
extern crate bytes;
#[macro_use]
extern crate cfg_if;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate log;
extern crate net2;
extern crate num_cpus;
extern crate thread_pool;
extern crate tokio_codec;
extern crate tokio_io;

#[doc(hidden)]
pub extern crate bincode;
#[doc(hidden)]
#[macro_use]
pub extern crate futures;
#[doc(hidden)]
pub extern crate serde;
#[doc(hidden)]
#[macro_use]
pub extern crate serde_derive;
#[doc(hidden)]
pub extern crate tokio_core;
#[doc(hidden)]
pub extern crate tokio_proto;
#[doc(hidden)]
pub extern crate tokio_service;

pub use errors::Error;
#[doc(hidden)]
pub use errors::WireError;

/// Provides some utility error types, as well as a trait for spawning futures on the default event
/// loop.
pub mod util;

/// Provides the macro used for constructing rpc services and client stubs.
#[macro_use]
mod macros;
/// Provides a few different error types.
mod errors;
/// Futures-based version of the tarpc API.
pub mod future;
/// Provides implementations of `ClientProto` and `ServerProto` that implement the tarpc protocol.
/// The tarpc protocol is a length-delimited, bincode-serialized payload.
mod protocol;
/// Provides an abstraction over TLS and TCP streams.
mod stream_type;
/// Synchronous version of the tarpc API
pub mod sync;
/// TLS-specific functionality.
#[cfg(feature = "tls")]
pub mod tls;

use std::sync::mpsc;
use std::thread;
use tokio_core::reactor;

lazy_static! {
    /// The `Remote` for the default reactor core.
    static ref REMOTE: reactor::Remote = {
        spawn_core()
    };
}

/// Spawns a `reactor::Core` running forever on a new thread.
fn spawn_core() -> reactor::Remote {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let mut core = reactor::Core::new().unwrap();
        tx.send(core.handle().remote().clone()).unwrap();

        // Run forever
        core.run(futures::empty::<(), !>()).unwrap();
    });
    rx.recv().unwrap()
}

cfg_if! {
    if #[cfg(feature = "tls")] {
        extern crate tokio_tls;
        extern crate native_tls as native_tls_inner;

        /// Re-exported TLS-related types from the `native_tls` crate.
        pub mod native_tls {
            pub use native_tls_inner::{Error, Pkcs12, TlsAcceptor, TlsConnector};
        }
    } else {}
}
