//! This creates a PFS repo called `hello-world`

extern crate pachyderm;
extern crate tokio;
extern crate tonic;

use std::env;
use std::error::Error;

use pachyderm::pfs::{api_client::ApiClient as PfsClient, CreateRepoRequest, Repo};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let mut args = env::args().collect::<Vec<String>>();
    let pachd_address = if args.len() > 1 {
        args.pop().unwrap()
    } else {
        "grpc://localhost:30650".into()
    };

    let mut client = PfsClient::connect(pachd_address.clone()).await?;

    let request = tonic::Request::new(CreateRepoRequest {
        repo: Some(Repo {
            name: "hello-world".into(),
        }),
        description: "".into(),
        update: false,
    });

    let response = client.create_repo(request).await?;

    println!("Response: {:?}", response);

    Ok(())
}
