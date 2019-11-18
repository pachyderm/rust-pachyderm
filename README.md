# Rust Pachyderm

[![Slack Status](http://slack.pachyderm.io/badge.svg)](http://slack.pachyderm.io)

Official Rust Pachyderm client. This library provides low-level (auto-generated) bindings to our gRPC services, with support for async/await thanks to [tonic](https://github.com/hyperium/tonic). It should work on rust stable 1.39+, as well as nightly/beta.

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

## Examples

- [Hello World](https://github.com/pachyderm/rust-pachyderm/blob/master/examples/hello_world.rs): Creates a PFS repo called `hello-world`. To run: `cargo run --example hello_world`
- [OpenCV](https://github.com/pachyderm/rust-pachyderm/blob/master/examples/opencv.rs): This is the [canonical Pachyderm/OpenCV demo](https://github.com/pachyderm/pachyderm/tree/master/examples/opencv), ported to this library. To run `cargo run --example opencv`
