#![no_main]

use std::env;

use pachyderm::pfs;
use pachyderm::pfs::api_client::ApiClient as PfsClient;
use pachyderm::pps::api_client::ApiClient as PpsClient;

use libfuzzer_sys::fuzz_target;
use arbitrary::{Arbitrary, Unstructured};
use tokio::runtime::Runtime;
use tonic::transport::Channel;
use tonic::{Code, Status};

async fn delete_all(pps_client: &mut PpsClient<Channel>, pfs_client: &mut PfsClient<Channel>) -> Result<(), Status> {
    pps_client.delete_all(()).await?;
    pfs_client.delete_all(()).await?;
    Ok(())
}

fn check<T>(result: Result<T, Status>) {
    if let Err(err) = result {
        match err.code() {
            Code::Cancelled | Code::DeadlineExceeded | Code::NotFound | Code::AlreadyExists | Code::PermissionDenied | Code::ResourceExhausted | Code::FailedPrecondition | Code::Aborted | Code::OutOfRange | Code::Unimplemented | Code::Internal | Code::Unavailable | Code::DataLoss | Code::Unauthenticated => {
                panic!("unexpected error: {:?}\n{}", err.code(), err);
            }
            _ => {
                println!("{}", err);
            }
        }
    }
}

#[derive(Arbitrary, Clone, Debug, PartialEq)]
struct Options {
    ops: Vec<Op>,
}

#[derive(Arbitrary, Clone, Debug, PartialEq)]
enum Op {
    CreateRepo { name: RepoName, update: bool },
    InspectRepo { name: RepoName },
    ListRepo,
    DeleteRepo { name: RepoName, force: bool, all: bool },
}

#[derive(Clone, Debug, PartialEq)]
struct RepoName {
    bytes: Vec<u8>
}

impl ToString for RepoName {
    fn to_string(&self) -> String {
        // Quick and dirty way to make valid repo names from arbitrary bytes.
        // As a small performance win, we could alternatively create a proper
        // `base64::Config` for this custom encoding.
        let s = base64::encode(&self.bytes);
        s.replace("+", "_").replace("/", "_").replace("=", "-")
    }
}

impl Arbitrary for RepoName {
    fn arbitrary(u: &mut Unstructured<'_>) -> arbitrary::Result<Self> {
        let bytes = Vec::<u8>::arbitrary(u)?;
        Ok(RepoName { bytes })
    }

    fn size_hint(_: usize) -> (usize, Option<usize>) {
        (1, None)
    }
}

async fn run(opts: Options) {
    let pachd_address = env::var("PACHD_ADDRESS").expect("No `PACHD_ADDRESS` set");
    let mut pfs_client = PfsClient::connect(pachd_address.clone()).await.unwrap();
    let mut pps_client = PpsClient::connect(pachd_address.clone()).await.unwrap();

    delete_all(&mut pps_client, &mut pfs_client).await.unwrap();

    for op in opts.ops.into_iter() {
        match op {
            Op::CreateRepo { name, update } => {
                check(pfs_client.create_repo(pfs::CreateRepoRequest {
                    repo: Some(pfs::Repo { name: name.to_string() }),
                    description: "".into(),
                    update: update,
                }).await);
            },

            Op::InspectRepo { name } => {
                check(pfs_client.inspect_repo(pfs::InspectRepoRequest {
                    repo: Some(pfs::Repo { name: name.to_string() }),
                }).await);
            },
            Op::ListRepo => {
                check(pfs_client.list_repo(pfs::ListRepoRequest {}).await);
            },

            Op::DeleteRepo { name, force, all } => {
                check(pfs_client.delete_repo(pfs::DeleteRepoRequest {
                    repo: Some(pfs::Repo { name: name.to_string() }),
                    force,
                    all
                }).await);
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
