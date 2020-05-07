#![no_main]

use std::env;

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
use tonic::{Code, Status};
use pretty_assertions::assert_eq;

const BASE_PIPELINE_CODE: &'static str = "
cp /pfs/fuzz_extract_restore_input/* /pfs/out/
test \"$(cat /pfs/out/test)\" != \"corrupted\"
";

const UPDATED_PIPELINE_CODE: &'static str = "
cp /pfs/fuzz_extract_restore_input/* /pfs/out/
";

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
    update: bool,
    stats: bool,
    reprocess: bool
) -> Result<(), Status> {
    let mut request = pps::CreatePipelineRequest::default();
    request.pipeline = Some(pps::Pipeline { name: name.into() });

    let mut transform = pps::Transform::default();
    transform.cmd = transform_cmd.into_iter().map(|i| i.into()).collect();
    if let Some(stdin) = transform_stdin {
        transform.stdin = vec![stdin.into()];
    }
    request.transform = Some(transform);
    request.input = Some(input);
    request.update = update;
    request.enable_stats = stats;
    request.reprocess = reprocess;

    pps_client.create_pipeline(request).await?;
    Ok(())
}

async fn delete_all(pps_client: &mut PpsClient<Channel>, pfs_client: &mut PfsClient<Channel>) -> Result<(), Status> {
    pps_client.delete_all(()).await?;
    pfs_client.delete_all(()).await?;
    Ok(())
}

async fn inspect_commit(pfs_client: &mut PfsClient<Channel>, commit: pfs::Commit) -> Result<pfs::CommitInfo, Code> {
    // just extract out the error code (if any), so that errors can be
    // compared with `assert_eq`
    let result = pfs_client.inspect_commit(
        pfs::InspectCommitRequest {
            commit: Some(commit),
            block_state: 0
        })
        .await
        .map_err(|status| status.code())?;
    let mut commit = result.into_inner();

    // these fields will change at extract/restore time, so don't compare them
    commit.started = None;
    commit.finished = None;

    // values are allowed to rearrange here, so we sort them
    commit.provenance.sort_unstable_by(|a, b| {
        match (a.commit.as_ref(), b.commit.as_ref()) {
            (Some(a), Some(b)) => a.id.cmp(&b.id),
            (None, _) | (_, None) => unreachable!(),
        }
    });
    
    Ok(commit)
}

async fn list_jobs(pps_client: &mut PpsClient<Channel>) -> Result<pps::JobInfos, Code> {
    // just extract out the error code (if any), so that errors can be
    // compared with `assert_eq`
    let result = pps_client.list_job(
        pps::ListJobRequest {
            pipeline: None,
            input_commit: Vec::default(),
            output_commit: None,
            history: -1,
            full: true
        })
        .await
        .map_err(|status| status.code())?;
    let jobs = result.into_inner();
    Ok(jobs)
}

async fn extract(admin_client: &mut AdminClient<Channel>, no_objects: bool, no_repos: bool, no_pipelines: bool) -> Result<Vec<admin::Op>, Status> {
    admin_client.extract(admin::ExtractRequest {
        url: "".to_string(),
        no_objects: no_objects,
        no_repos,
        no_pipelines,
    }).await.unwrap().into_inner().try_collect::<Vec<admin::Op>>().await
}

#[derive(Arbitrary, Clone, Debug, PartialEq)]
struct Options {
    deferred: bool,
    ops: Vec<Op>,
    outro: Outro
}

#[derive(Arbitrary, Clone, Debug, PartialEq)]
enum Op {
    PutFile { corrupted: bool, flush: bool },
    UpdatePipeline { stats: bool, reprocess: bool }
}

#[derive(Arbitrary, Clone, Debug, PartialEq)]
enum Outro {
    Extract { no_objects: bool, no_repos: bool, no_pipelines: bool },
    ExtractRestore { no_objects: bool },
}

async fn run(opts: Options) {
    let pachd_address = env::var("PACHD_ADDRESS").expect("No `PACHD_ADDRESS` set");
    let mut counter = 0;
    let mut inserted_file = false;
    let mut last_corrupted = false;
    let mut pfs_client = PfsClient::connect(pachd_address.clone()).await.unwrap();
    let mut pps_client = PpsClient::connect(pachd_address.clone()).await.unwrap();
    let mut admin_client = AdminClient::connect(pachd_address.clone()).await.unwrap();

    delete_all(&mut pps_client, &mut pfs_client).await.unwrap();

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
        Some(BASE_PIPELINE_CODE),
        create_pfs_input("/*", "fuzz_extract_restore_input"),
        false,
        false,
        false,
    ).await.unwrap();

    let input_head_commit = pfs::Commit {
        repo: Some(pfs::Repo {
            name: "fuzz_extract_restore_input".to_string(),
        }),
        id: if opts.deferred {
            "staging".to_string()
        } else {
            "master".to_string()
        },
    };

    let output_head_commit = pfs::Commit {
        repo: Some(pfs::Repo {
            name: "fuzz_extract_restore_output".to_string(),
        }),
        id: "master".to_string(),
    };

    for op in opts.ops.into_iter() {
        match op {
            Op::PutFile { corrupted, flush } => {
                let contents = if corrupted {
                    last_corrupted = true;
                    "corrupted".to_string()
                } else {
                    let value = counter;
                    counter += 1;
                    last_corrupted = false;
                    format!("{}", value)
                };

                let mut req = pfs::PutFileRequest::default();
                req.file = Some(pfs::File {
                    path: "/test".to_string(),
                    commit: Some(input_head_commit.clone()),
                });

                req.value = contents.into_bytes();
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

                inserted_file = true;
            },
            Op::UpdatePipeline { stats, reprocess } => {
                create_pipeline(
                    &mut pps_client,
                    "fuzz_extract_restore_output",
                    vec!["bash"],
                    Some(UPDATED_PIPELINE_CODE),
                    create_pfs_input("/*", "fuzz_extract_restore_input"),
                    true,
                    stats,
                    reprocess
                ).await.unwrap();
            }
        }

    }

    if opts.deferred {
        pfs_client.create_branch(pfs::CreateBranchRequest {
            head: Some(input_head_commit.clone()),
            branch: Some(pfs::Branch {
                repo: Some(pfs::Repo {
                    name: "fuzz_extract_restore_input".to_string()
                }),
                name: "master".to_string()
            }),
            s_branch: "".to_string(),
            provenance: Vec::default(),
        }).await.unwrap();
    }

    match opts.outro {
        Outro::Extract { no_objects, no_repos, no_pipelines } => {
            extract(&mut admin_client, no_objects, no_repos, no_pipelines).await.unwrap();
        },
        Outro::ExtractRestore { no_objects } => {
            let input_commit_before = inspect_commit(&mut pfs_client, input_head_commit.clone()).await;
            let output_commit_before = inspect_commit(&mut pfs_client, output_head_commit.clone()).await;
            let jobs_before = list_jobs(&mut pps_client).await;

            let extracted = extract(&mut admin_client, no_objects, false, false).await.unwrap();

            // restoring on top of a non-empty cluster is undefined behavior, so clear
            // everything out before restoring
            delete_all(&mut pps_client, &mut pfs_client).await.unwrap();
            if !no_objects {
                pps_client.garbage_collect(pps::GarbageCollectRequest {
                    memory_bytes: 64*1024*1024,
                }).await.unwrap();
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

            // ensure commits remain the same
            let input_commit_after = inspect_commit(&mut pfs_client, input_head_commit.clone()).await;
            let output_commit_after = inspect_commit(&mut pfs_client, output_head_commit.clone()).await;
            let jobs_after = list_jobs(&mut pps_client).await;
            assert_eq!(input_commit_before, input_commit_after);
            assert_eq!(output_commit_before, output_commit_after);
            assert_eq!(jobs_before, jobs_after);

            if inserted_file {
                // ensure the file contents are restored
                let file_bytes: Vec<Vec<u8>> = pfs_client.get_file(pfs::GetFileRequest {
                    file: Some(pfs::File {
                        commit: Some(output_head_commit),
                        path: "/test".to_string(),
                    }),
                    offset_bytes: 0,
                    size_bytes: 0,
                }).await.unwrap().into_inner().try_collect::<Vec<Vec<u8>>>().await.unwrap();
                let file_bytes = file_bytes.into_iter().flatten().collect::<Vec<u8>>();

                if last_corrupted {
                    assert_eq!(file_bytes, "corrupted".to_string().into_bytes());
                } else {
                    assert_eq!(file_bytes, format!("{}", counter - 1).into_bytes());
                }
            }
        }
    }

    delete_all(&mut pps_client, &mut pfs_client).await.unwrap();
}

fuzz_target!(|opts: Options| {
    if opts.ops.is_empty() {
        return;
    }
    Runtime::new().unwrap().block_on(run(opts));
});
