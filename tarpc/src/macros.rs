// Copyright 2016 Google Inc. All Rights Reserved.
//
// Licensed under the MIT License, <LICENSE or http://opensource.org/licenses/MIT>.
// This file may not be copied, modified, or distributed except according to those terms.

/// The main macro that creates RPC services.
///
/// Rpc methods are specified, mirroring trait syntax:
///
/// ```
/// # #![feature(plugin, await_macro, async_await, existential_type, futures_api)]
/// # #![plugin(tarpc_plugins)]
/// # #[macro_use] extern crate tarpc;
/// # fn main() {}
/// # service! {
/// /// Say hello
/// rpc hello(name: String) -> String;
/// # }
/// ```
///
/// Attributes can be attached to each rpc. These attributes
/// will then be attached to the generated service traits'
/// corresponding `fn`s, as well as to the client stubs' RPCs.
///
/// The following items are expanded in the enclosing module:
///
/// * `Service` -- the trait defining the RPC service via a `Future` API.
/// * `ServiceExt` -- provides the methods for starting a service. There is an umbrella impl
///                         for all implers of `Service`. It's a separate trait to prevent
///                         name collisions with RPCs.
/// * `Client` -- a client whose RPCs return `Future`s.
///
#[macro_export]
macro_rules! service {
// Entry point
    (
        $(
            $(#[$attr:meta])*
            rpc $fn_name:ident( $( $arg:ident : $in_:ty ),* ) $(-> $out:ty)*;
        )*
    ) => {
        service! {{
            $(
                $(#[$attr])*
                rpc $fn_name( $( $arg : $in_ ),* ) $(-> $out)*;
            )*
        }}
    };
// Pattern for when the next rpc has an implicit unit return type.
    (
        {
            $(#[$attr:meta])*
            rpc $fn_name:ident( $( $arg:ident : $in_:ty ),* );

            $( $unexpanded:tt )*
        }
        $( $expanded:tt )*
    ) => {
        service! {
            { $( $unexpanded )* }

            $( $expanded )*

            $(#[$attr])*
            rpc $fn_name( $( $arg : $in_ ),* ) -> ();
        }
    };
// Pattern for when the next rpc has an explicit return type.
    (
        {
            $(#[$attr:meta])*
            rpc $fn_name:ident( $( $arg:ident : $in_:ty ),* ) -> $out:ty;

            $( $unexpanded:tt )*
        }
        $( $expanded:tt )*
    ) => {
        service! {
            { $( $unexpanded )* }

            $( $expanded )*

            $(#[$attr])*
            rpc $fn_name( $( $arg : $in_ ),* ) -> $out;
        }
    };
// Pattern for when all return types have been expanded
    (
        { } // none left to expand
        $(
            $(#[$attr:meta])*
            rpc $fn_name:ident ( $( $arg:ident : $in_:ty ),* ) -> $out:ty;
        )*
    ) => {

        #[derive(Debug)]
        #[doc(hidden)]
        #[allow(non_camel_case_types, unused)]
        #[derive($crate::serde_derive::Serialize, $crate::serde_derive::Deserialize)]
        pub enum Request__ {
            $(
                $fn_name{ $($arg: $in_,)* }
            ),*
        }

        #[derive(Debug)]
        #[doc(hidden)]
        #[allow(non_camel_case_types, unused)]
        #[derive($crate::serde_derive::Serialize, $crate::serde_derive::Deserialize)]
        pub enum Response__ {
            $(
                $fn_name($out)
            ),*
        }

/// Defines the `Future` RPC service. Implementors must be `Clone` and `'static`,
/// as required by `tokio_proto::NewService`. This is required so that the service can be used
/// to respond to multiple requests concurrently.
        pub trait Service: Clone + Send + 'static {
            $(
                snake_to_camel! {
                    /// The type of future returned by `{}`.
                    type $fn_name: $crate::futures::Future<Output = $out> + Send;
                }

                $(#[$attr])*
                fn $fn_name(&self, ctx: $crate::rpc::context::Context, $($arg:$in_),*) -> ty_snake_to_camel!(Self::$fn_name);
            )*
        }

        existential type Resp<S>: ::std::future::Future<Output=::std::io::Result<Response__>> + Send;

        /// Returns a serving function to use with rpc::server::Server.
        pub fn serve<S: Service>(service: S)
            -> impl FnMut($crate::rpc::context::Context, Request__) -> Resp<S> + Send + 'static + Clone {
                move |ctx, req| {
                    let mut service = service.clone();
                    async move {
                        match req {
                            $(
                                Request__::$fn_name{ $($arg,)* } => {
                                    let resp = Service::$fn_name(&mut service, ctx, $($arg),*);
                                    let resp = await!(resp);
                                    Ok(Response__::$fn_name(resp))
                                }
                            )*
                        }
                    }
                }
            }

        #[allow(unused)]
        #[derive(Clone, Debug)]
        /// The client stub that makes RPC calls to the server. Exposes a Future interface.
        pub struct Client($crate::rpc::client::Client<Request__, Response__>);

        /// Returns a new client stub that sends requests over the given transport.
        pub async fn new_stub<T>(config: $crate::rpc::client::Config, transport: T) -> Client
        where
            T: $crate::rpc::Transport<
                    Item = $crate::rpc::Response<Response__>,
                    SinkItem = $crate::rpc::ClientMessage<Request__>> + Send,
        {
            Client(await!($crate::rpc::client::Client::new(config, transport)))
        }

        impl Client {
            $(
                #[allow(unused)]
                $(#[$attr])*
                pub fn $fn_name(&mut self, ctx: $crate::rpc::context::Context, $($arg: $in_),*)
                    -> impl ::std::future::Future<Output = ::std::io::Result<$out>> + '_ {
                    let request__ = Request__::$fn_name { $($arg,)* };
                    let resp = self.0.send(ctx, request__);
                    async move {
                        match await!(resp)? {
                            Response__::$fn_name(msg__) => ::std::result::Result::Ok(msg__),
                            _ => unreachable!(),
                        }
                    }
                }
            )*
        }
    }
}

// allow dead code; we're just testing that the macro expansion compiles
#[allow(dead_code)]
#[cfg(test)]
mod syntax_test {
    service! {
        #[deny(warnings)]
        #[allow(non_snake_case)]
        rpc TestCamelCaseDoesntConflict();
        rpc hello() -> String;
        #[doc="attr"]
        rpc attr(s: String) -> String;
        rpc no_args_no_return();
        rpc no_args() -> ();
        rpc one_arg(foo: String) -> i32;
        rpc two_args_no_return(bar: String, baz: u64);
        rpc two_args(bar: String, baz: u64) -> String;
        rpc no_args_ret_error() -> i32;
        rpc one_arg_ret_error(foo: String) -> String;
        rpc no_arg_implicit_return_error();
        #[doc="attr"]
        rpc one_arg_implicit_return_error(foo: String);
    }
}

#[cfg(test)]
mod functional_test {
    use futures::compat::TokioDefaultSpawner;
    use futures::future::{ready, Ready};
    use futures::prelude::*;
    use rpc::{
        client, context,
        server::{self, Handler},
        transport::channel,
    };
    use std::io;

    service! {
        rpc add(x: i32, y: i32) -> i32;
        rpc hey(name: String) -> String;
    }

    #[derive(Clone)]
    struct Server;

    impl Service for Server {
        type AddFut = Ready<i32>;

        fn add(&self, _: context::Context, x: i32, y: i32) -> Self::AddFut {
            ready(x + y)
        }

        type HeyFut = Ready<String>;

        fn hey(&self, _: context::Context, name: String) -> Self::HeyFut {
            ready(format!("Hey, {}.", name))
        }
    }

    #[test]
    fn sequential() {
        let _ = env_logger::try_init();

        let test = async {
            let (tx, rx) = channel::unbounded();
            spawn!(
                rpc::Server::new(server::Config::default())
                    .incoming(stream::once(ready(Ok(rx))))
                    .respond_with(serve(Server))
            );

            let mut client = await!(new_stub(client::Config::default(), tx));
            assert_eq!(3, await!(client.add(context::current(), 1, 2))?);
            assert_eq!(
                "Hey, Tim.",
                await!(client.hey(context::current(), "Tim".to_string()))?
            );
            Ok::<_, io::Error>(())
        }
            .map_err(|e| panic!(e.to_string()));

        tokio::run(test.boxed().compat(TokioDefaultSpawner));
    }

    #[test]
    fn concurrent() {
        let _ = env_logger::try_init();

        let test = async {
            let (tx, rx) = channel::unbounded();
            spawn!(
                rpc::Server::new(server::Config::default())
                    .incoming(stream::once(ready(Ok(rx))))
                    .respond_with(serve(Server))
            );

            let client = await!(new_stub(client::Config::default(), tx));
            let mut c = client.clone();
            let req1 = c.add(context::current(), 1, 2);
            let mut c = client.clone();
            let req2 = c.add(context::current(), 3, 4);
            let mut c = client.clone();
            let req3 = c.hey(context::current(), "Tim".to_string());

            assert_eq!(3, await!(req1)?);
            assert_eq!(7, await!(req2)?);
            assert_eq!("Hey, Tim.", await!(req3)?);
            Ok::<_, io::Error>(())
        }
            .map_err(|e| panic!(e.to_string()));

        tokio::run(test.boxed().compat(TokioDefaultSpawner));
    }
}
