# Rust Pachyderm

[![Slack Status](http://slack.pachyderm.io/badge.svg)](http://slack.pachyderm.io)

Official Rust Pachyderm client. This library provides low-level (auto-generated) bindings to our gRPC services.

## A Small Taste

Here's an example that creates a repo and adds a file:

```rust
//! This creates a PFS repo called `hello-world`

extern crate pachyderm;
extern crate tokio;
extern crate tonic;

use std::error::Error;

use pachyderm::pfs::{client::ApiClient as PfsClient, CreateRepoRequest, Repo};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let mut client = PfsClient::connect("http://localhost:30650").await?;

    let request = tonic::Request::new(CreateRepoRequest {
        repo: Some(Repo {
            name: "hello-world".into()
        }),
        description: "".into(),
        update: false
    });

    let response = client.create_repo(request).await?;

    println!("Response: {:?}", response);

    Ok(())
}
```

For more sophisticated examples, [see the examples directory](https://github.com/pachyderm/rust-pachyderm/tree/master/examples).
