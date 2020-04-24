#![no_main]

extern crate tokio;

use std::env;
use std::error::Error;

use libfuzzer_sys::fuzz_target;
use arbitrary::{Arbitrary, Unstructured};
use pachyderm::pfs::{api_client::ApiClient as PfsClient, CreateRepoRequest, Repo};
use futures::executor::block_on;

#[derive(Arbitrary, Clone, Debug, PartialEq)]
pub enum Op {
    Foo,
    Bar
}

async fn run(ops: Vec<Op>) -> Result<(), Box<dyn Error>> {
    let pachd_address = env::var("PACHD_ADDRESS")?;

    let mut client = PfsClient::connect(pachd_address.clone()).await?;

    let request = tonic::Request::new(CreateRepoRequest {
        // repo: Some(Repo {
        //     name: "hello-world".into(),
        // }),
        repo: None,
        description: "".into(),
        update: false,
    });

    let response = client.create_repo(request).await?;

    println!("Response: {:?}", response);

    Ok(())
}

// #[tokio::main]
fuzz_target!(|ops: Vec<Op>| {
    block_on(run(ops)).unwrap();
});
