#![no_main]

#[macro_use]
extern crate lazy_static;

use std::env;
use std::error::Error;
use std::sync::atomic::{AtomicUsize, Ordering};

use pachyderm::pfs;
use pachyderm::pfs::api_client::ApiClient as PfsClient;
// use pachyderm::pps::api_client::ApiClient as PpsClient;
use pachyderm::admin;
use pachyderm::admin::api_client::ApiClient as AdminClient;

use libfuzzer_sys::fuzz_target;
use arbitrary::Arbitrary;
use tokio::runtime::Runtime;
use futures::stream;
use futures::stream::TryStreamExt;

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
    let mut pfs_client = PfsClient::connect(pachd_address.clone()).await?;
    let mut admin_client = AdminClient::connect(pachd_address.clone()).await?;
    
    for op in ops {
        match op {
            Op::PutFile { flush } => {
                let head_commit = pfs::Commit {
                    repo: Some(pfs::Repo {
                        name: "fuzz_extract_restore_input".to_string(),
                    }),
                    id: "master".to_string(),
                };

                let mut req = pfs::PutFileRequest::default();
                req.file = Some(pfs::File {
                    path: "/test".to_string(),
                    commit: Some(head_commit.clone()),
                });
                req.value = format!("{}", (*COUNTER).fetch_add(1, Ordering::Relaxed)).into_bytes();
                req.overwrite_index = Some(pfs::OverwriteIndex {
                    index: 0
                });

                let req_stream = stream::iter(vec![req]);
                pfs_client.put_file(req_stream).await?;

                if flush {
                    pfs_client.flush_commit(pfs::FlushCommitRequest {
                        commits: vec![head_commit],
                        to_repos: vec![],
                    }).await?;
                }
            },
            Op::ExtractRestore { no_objects, no_repos, no_pipelines } => {
                let extracted: Vec<admin::Op> = admin_client.extract(admin::ExtractRequest {
                    url: "".to_string(),
                    no_objects,
                    no_repos,
                    no_pipelines,
                }).await?.into_inner().try_collect::<Vec<admin::Op>>().await?;

                let reqs: Vec<admin::RestoreRequest> = extracted.into_iter().map(|op| {
                    admin::RestoreRequest{
                        op: Some(op),
                        url: "".to_string()
                    }
                }).collect();

                admin_client.restore(stream::iter(reqs)).await?;
            }
        }
    }

    Ok(())
}

fuzz_target!(|ops: Vec<Op>| {
    Runtime::new().unwrap().block_on(run(ops)).unwrap();
});
