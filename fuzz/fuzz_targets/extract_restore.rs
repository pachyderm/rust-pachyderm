#![no_main]

#[macro_use]
extern crate lazy_static;

use std::env;
use std::error::Error;
use std::sync::atomic::AtomicUsize;

use pachyderm::pfs;
use pachyderm::pfs::api_client::ApiClient as PfsClient;
use pachyderm::pps;
// use pachyderm::pps::api_client::ApiClient as PpsClient;
use pachyderm::admin::api_client::ApiClient as AdminClient;

use libfuzzer_sys::fuzz_target;
use arbitrary::{Arbitrary, Unstructured};
use tokio::runtime::Runtime;

lazy_static! {
    static ref COUNTER: AtomicUsize = AtomicUsize::new(0);
}

#[derive(Arbitrary, Clone, Debug, PartialEq)]
pub enum Op {
    PutFile { flush: bool },
    ExtractRestore { no_objects: bool, no_repos: bool, no_pipelines: bool },
}

async fn run(ops: Vec<Op>) -> Result<(), Box<dyn Error>> {
    let pachd_address = env::var("PACHD_ADDRESS")?;
    let pfs_client = PfsClient::connect(pachd_address.clone()).await?;
    let admin_client = AdminClient::connect(pachd_address.clone()).await?;
    
    for op in ops {
        match op {
            Op::PutFile { flush } => {
                //
            },
            Op::ExtractRestore { no_objects, no_repos, no_pipelines } => {
                //
            }
        }
    }

    Ok(())
}

fuzz_target!(|ops: Vec<Op>| {
    Runtime::new().unwrap().block_on(run(ops)).unwrap();
});
