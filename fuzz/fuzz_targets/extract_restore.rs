#![no_main]

#[macro_use]
extern crate lazy_static;

use std::env;
use std::sync::atomic::{AtomicUsize, Ordering};

use pachyderm::pfs;
use pachyderm::pfs::api_client::ApiClient as PfsClient;
use pachyderm::pps::api_client::ApiClient as PpsClient;
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
    ExtractRestore { no_objects: bool, no_repos: bool, no_pipelines: bool, delete_all: bool },
}

async fn run(op: Op) {
    let pachd_address = env::var("PACHD_ADDRESS").expect("No `PACHD_ADDRESS` set");
    let mut pfs_client = PfsClient::connect(pachd_address.clone()).await.unwrap();

    let input_head_commit = pfs::Commit {
        repo: Some(pfs::Repo {
            name: "fuzz_extract_restore_input".to_string(),
        }),
        id: "master".to_string(),
    };

    let output_head_commit = pfs::Commit {
        repo: Some(pfs::Repo {
            name: "fuzz_extract_restore_output".to_string(),
        }),
        id: "master".to_string(),
    };
    
    match op {
        Op::PutFile { flush } => {
            let mut req = pfs::PutFileRequest::default();
            req.file = Some(pfs::File {
                path: "/test".to_string(),
                commit: Some(input_head_commit.clone()),
            });
            req.value = format!("{}", (*COUNTER).fetch_add(1, Ordering::Relaxed)).into_bytes();
            req.overwrite_index = Some(pfs::OverwriteIndex {
                index: 0
            });

            let req_stream = stream::iter(vec![req]);
            pfs_client.put_file(req_stream).await.unwrap();

            if flush {
                pfs_client.flush_commit(pfs::FlushCommitRequest {
                    commits: vec![input_head_commit],
                    to_repos: vec![],
                }).await.unwrap();
            }
        },
        Op::ExtractRestore { no_objects, no_repos, no_pipelines, delete_all } => {
            let mut pps_client = PpsClient::connect(pachd_address.clone()).await.unwrap();
            let mut admin_client = AdminClient::connect(pachd_address.clone()).await.unwrap();

            let input_commit_before = pfs_client.inspect_commit(pfs::InspectCommitRequest {
                commit: Some(input_head_commit.clone()),
                block_state: 0
            }).await.unwrap().into_inner();
            let output_commit_before = pfs_client.inspect_commit(pfs::InspectCommitRequest {
                commit: Some(output_head_commit.clone()),
                block_state: 0
            }).await.unwrap().into_inner();

            let extracted: Vec<admin::Op> = admin_client.extract(admin::ExtractRequest {
                url: "".to_string(),
                no_objects,
                no_repos,
                no_pipelines,
            }).await.unwrap().into_inner().try_collect::<Vec<admin::Op>>().await.unwrap();

            if delete_all {
                pps_client.delete_all(()).await.unwrap();
            }

            let reqs: Vec<admin::RestoreRequest> = extracted.into_iter().map(|op| {
                admin::RestoreRequest{
                    op: Some(op),
                    url: "".to_string()
                }
            }).collect();
            admin_client.restore(stream::iter(reqs)).await.unwrap();

            // ensure it passes fsck
            pfs_client.fsck(pfs::FsckRequest { fix: false }).await.unwrap();

            // // TODO: ensure we have jobs, file contents preserved
            let input_commit_after = pfs_client.inspect_commit(pfs::InspectCommitRequest {
                commit: Some(input_head_commit.clone()),
                block_state: 0
            }).await.unwrap().into_inner();
            let output_commit_after = pfs_client.inspect_commit(pfs::InspectCommitRequest {
                commit: Some(output_head_commit.clone()),
                block_state: 0
            }).await.unwrap().into_inner();

            assert_eq!(input_commit_before, input_commit_after);
            assert_eq!(output_commit_before, output_commit_after);
        }
    }
}

fuzz_target!(|op: Op| {
    Runtime::new().unwrap().block_on(run(op));
});
