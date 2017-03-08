// Copyright 2016 Google Inc. All Rights Reserved.
//
// Licensed under the MIT License, <LICENSE or http://opensource.org/licenses/MIT>.
// This file may not be copied, modified, or distributed except according to those terms.

use REMOTE;
use futures::{self, Future, future};
use protocol::Proto;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::io;
use std::net::SocketAddr;
use stream_type::StreamType;
use tokio_core::net::TcpStream;
use tokio_core::reactor;
use tokio_proto::BindClient as ProtoBindClient;
use tokio_proto::multiplex::ClientService;
use tokio_service::Service;

cfg_if! {
    if #[cfg(feature = "tls")] {
        use errors::native_to_io;
        use tls::client::Context;
        use tokio_tls::TlsConnectorExt;
    } else {}
}

/// Additional options to configure how the client connects and operates.
#[derive(Default)]
pub struct Options {
    reactor: Option<Reactor>,
    #[cfg(feature = "tls")]
    tls_ctx: Option<Context>,
}

impl Options {
    /// Drive using the given reactor handle. Only used by `FutureClient`s.
    pub fn handle(mut self, handle: reactor::Handle) -> Self {
        self.reactor = Some(Reactor::Handle(handle));
        self
    }

    /// Drive using the given reactor remote. Only used by `FutureClient`s.
    pub fn remote(mut self, remote: reactor::Remote) -> Self {
        self.reactor = Some(Reactor::Remote(remote));
        self
    }

    /// Connect using the given `Context`
    #[cfg(feature = "tls")]
    pub fn tls(mut self, tls_ctx: Context) -> Self {
        self.tls_ctx = Some(tls_ctx);
        self
    }
}

enum Reactor {
    Handle(reactor::Handle),
    Remote(reactor::Remote),
}

#[doc(hidden)]
pub struct Client<Req, Resp, E> {
    inner: ClientService<Req, Result<Resp, E>>,
}

impl<Req, Resp, E> Clone for Client<Req, Resp, E> {
    fn clone(&self) -> Self {
        Client { inner: self.inner.clone() }
    }
}

impl<Req, Resp, E> Service for Client<Req, Resp, E>
    where Req: Serialize + Sync + Send + 'static,
          Resp: Deserialize + Sync + Send + 'static,
          E: Deserialize + Sync + Send + 'static
{
    type Request = Req;
    type Response = Resp;
    type Error = ::Error<E>;
    type Future = ResponseFuture<Req, Resp, E>;

    fn call(&self, request: Self::Request) -> Self::Future {
        fn identity<T>(t: T) -> T {
            t
        }
        self.inner
            .call(request)
            .map(Self::map_err as _)
            .map_err(::Error::from as _)
            .and_then(identity as _)
    }
}

impl<Req, Resp, E> Client<Req, Resp, E> {
    fn bind(handle: &reactor::Handle, tcp: StreamType) -> Self
        where Req: Serialize + Sync + Send + 'static,
              Resp: Deserialize + Sync + Send + 'static,
              E: Deserialize + Sync + Send + 'static
    {
        let inner = Proto::new().bind_client(&handle, tcp);
        Client { inner }
    }

    fn map_err(resp: Result<Resp, E>) -> Result<Resp, ::Error<E>> {
        resp.map_err(::Error::App)
    }
}

impl<Req, Resp, E> fmt::Debug for Client<Req, Resp, E> {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(f, "Client {{ .. }}")
    }
}

/// Extension methods for clients.
pub trait ClientExt: Sized {
    /// The type of the future returned when calling `connect`.
    type ConnectFut: Future<Item = Self, Error = io::Error>;

    /// Connects to a server located at the given address, using the given options.
    fn connect(addr: SocketAddr, options: Options) -> Self::ConnectFut;
}

/// A future that resolves to a `Client` or an `io::Error`.
pub type ConnectFuture<Req, Resp, E> =
    futures::Flatten<futures::MapErr<futures::Oneshot<io::Result<Client<Req, Resp, E>>>,
                                     fn(futures::Canceled) -> io::Error>>;

impl<Req, Resp, E> ClientExt for Client<Req, Resp, E>
    where Req: Serialize + Sync + Send + 'static,
          Resp: Deserialize + Sync + Send + 'static,
          E: Deserialize + Sync + Send + 'static
{
    type ConnectFut = ConnectFuture<Req, Resp, E>;

    fn connect(addr: SocketAddr, options: Options) -> Self::ConnectFut {
        // we need to do this for tls because we need to avoid moving the entire `Options`
        // struct into the `setup` closure, since `Reactor` is not `Send`.
        #[cfg(feature = "tls")]
        let mut options = options;
        #[cfg(feature = "tls")]
        let tls_ctx = options.tls_ctx.take();

        let connect = move |handle: &reactor::Handle| {
            let handle2 = handle.clone();
            TcpStream::connect(&addr, handle)
                .and_then(move |socket| {
                    // TODO(https://github.com/tokio-rs/tokio-proto/issues/132): move this into the
                    // ServerProto impl
                    #[cfg(feature = "tls")]
                    match tls_ctx {
                        Some(tls_ctx) => {
                            future::Either::A(tls_ctx.tls_connector
                                .connect_async(&tls_ctx.domain, socket)
                                .map(StreamType::Tls)
                                .map_err(native_to_io))
                        }
                        None => future::Either::B(future::ok(StreamType::Tcp(socket))),
                    }
                    #[cfg(not(feature = "tls"))]
                    future::ok(StreamType::Tcp(socket))
                })
                .map(move |tcp| Client::bind(&handle2, tcp))
        };
        let (tx, rx) = futures::oneshot();
        let setup = move |handle: &reactor::Handle| {
            connect(handle).then(move |result| {
                tx.complete(result);
                Ok(())
            })
        };

        match options.reactor {
            Some(Reactor::Handle(handle)) => {
                handle.spawn(setup(&handle));
            }
            Some(Reactor::Remote(remote)) => {
                remote.spawn(setup);
            }
            None => {
                REMOTE.spawn(setup);
            }
        }
        fn panic(canceled: futures::Canceled) -> io::Error {
            unreachable!(canceled)
        }
        rx.map_err(panic as _).flatten()
    }
}

type ResponseFuture<Req, Resp, E> =
    futures::AndThen<futures::MapErr<
    futures::Map<<ClientService<Req, Result<Resp, E>> as Service>::Future,
                 fn(Result<Resp, E>) -> Result<Resp, ::Error<E>>>,
        fn(io::Error) -> ::Error<E>>,
                 Result<Resp, ::Error<E>>,
                 fn(Result<Resp, ::Error<E>>) -> Result<Resp, ::Error<E>>>;
