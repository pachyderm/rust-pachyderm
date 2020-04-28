#![no_main]

#[macro_use]
extern crate lazy_static;

use std::env;
use std::error::Error;
use std::sync::atomic::{AtomicUsize, Ordering};

use pachyderm::pfs;
use pachyderm::pfs::api_client::ApiClient as PfsClient;
use pachyderm::pps;
use pachyderm::pps::api_client::ApiClient as PpsClient;
use pachyderm::admin;
use pachyderm::admin::api_client::ApiClient as AdminClient;

use libfuzzer_sys::fuzz_target;
use arbitrary::Arbitrary;
use tokio::runtime::Runtime;
use futures::stream;
use futures::stream::TryStreamExt;
use tonic::transport::Channel;

lazy_static! {
    static ref COUNTER: AtomicUsize = AtomicUsize::new(0);
}

fn create_pfs_input(glob: &str, repo: &str) -> pps::Input {
    let mut pfs_input = pps::PfsInput::default();
    pfs_input.glob = glob.into();
    pfs_input.repo = repo.into();

    let mut input = pps::Input::default();
    input.pfs = Some(pfs_input);

    input
}

async fn create_pipeline(
    pps_client: &mut PpsClient<Channel>,
    name: &str,
    transform_cmd: Vec<&str>,
    transform_stdin: Option<&str>,
    input: pps::Input,
) -> Result<(), Box<dyn Error>> {
    let mut request = pps::CreatePipelineRequest::default();
    request.pipeline = Some(pps::Pipeline { name: name.into() });

    let mut transform = pps::Transform::default();
    transform.cmd = transform_cmd.into_iter().map(|i| i.into()).collect();
    if let Some(stdin) = transform_stdin {
        transform.stdin = vec![stdin.into()];
    }
    request.transform = Some(transform);
    request.input = Some(input);

    pps_client.create_pipeline(request).await?;
    Ok(())
}

#[derive(Arbitrary, Clone, Debug, PartialEq)]
pub enum Op {
    PutFile { flush: bool },
    ExtractRestore { no_objects: bool, no_repos: bool, no_pipelines: bool, delete_all: bool },
}

async fn run(ops: Vec<Op>) {
    let pachd_address = env::var("PACHD_ADDRESS").expect("No `PACHD_ADDRESS` set");
    let mut pfs_client = PfsClient::connect(pachd_address.clone()).await.unwrap();
    let mut pps_client = PpsClient::connect(pachd_address.clone()).await.unwrap();
    let mut admin_client = AdminClient::connect(pachd_address.clone()).await.unwrap();

    pps_client.delete_all(()).await.unwrap();
    pfs_client.delete_all(()).await.unwrap();

    pfs_client.create_repo(pfs::CreateRepoRequest {
        repo: Some(pfs::Repo {
            name: "fuzz_extract_restore_input".into(),
        }),
        description: "".into(),
        update: false,
    }).await.unwrap();

    create_pipeline(
        &mut pps_client,
        "fuzz_extract_restore_output",
        vec!["bash"],
        Some("cp /pfs/fuzz_extract_restore_input/* /pfs/out/"),
        create_pfs_input("/*", "fuzz_extract_restore_input")
    ).await.unwrap();

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

    println!("pass");

    for op in ops.into_iter() {
        println!("- {:?}", op);

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
                        commits: vec![input_head_commit.clone()],
                        to_repos: vec![],
                    }).await.unwrap();
                }
            },
            Op::ExtractRestore { no_objects, no_repos, no_pipelines, delete_all } => {
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
                    pfs_client.delete_all(()).await.unwrap();
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

    pps_client.delete_all(()).await.unwrap();
    pfs_client.delete_all(()).await.unwrap();
}

fuzz_target!(|ops: Vec<Op>| {
    Runtime::new().unwrap().block_on(run(ops));
});
