//! This creates a PFS repo called `hello-world`

extern crate futures;
extern crate http;
extern crate pachyderm;
extern crate tokio;
extern crate tower_add_origin;
extern crate tower_h2;
extern crate tower_grpc;
extern crate tower_service;
extern crate tower_util;

use std::io::Error as IoError;

use futures::{Future, Poll};
use http::Uri;
use tokio::executor::DefaultExecutor;
use tokio::net::tcp::{ConnectFuture, TcpStream};
use tower_add_origin::Builder;
use tower_grpc::Request;
use tower_h2::client;
use tower_service::Service;
use tower_util::MakeService;

use pachyderm::pfs::{client::Api as PfsApi, CreateRepoRequest, Repo};

fn main() {
    let h2_settings = Default::default();
    let mut make_client = client::Connect::new(Dest, h2_settings, DefaultExecutor::current());
    let uri: Uri = "http://localhost:30650".parse().unwrap();

    let callback = make_client
        .make_service(())
        .map(move |conn| {
            let conn = Builder::new().uri(uri).build(conn).unwrap();

            PfsApi::new(conn)
        })
        .and_then(|mut client| {
            client
                .create_repo(Request::new(CreateRepoRequest {
                    repo: Some(Repo {
                        name: "hello-world".to_string(),
                    }),
                    description: "".to_string(),
                    update: false,
                }))
                .map_err(|err| panic!("gRPC request failed: {:?}", err))
        })
        .and_then(|response| {
            println!("Response: {:?}", response);
            Ok(())
        })
        .map_err(|err| {
            eprintln!("PFS error: {:?}", err);
        });

    tokio::run(callback);
}

struct Dest;

impl Service<()> for Dest {
    type Response = TcpStream;
    type Error = IoError;
    type Future = ConnectFuture;

    fn poll_ready(&mut self) -> Poll<(), Self::Error> {
        Ok(().into())
    }

    fn call(&mut self, _: ()) -> Self::Future {
        TcpStream::connect(&([127, 0, 0, 1], 30650).into())
    }
}
