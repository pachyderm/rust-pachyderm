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

impl Options {
    fn valid(&self) -> bool {
        if self.ops.is_empty() || !self.ops.iter().all(|op| op.valid()) {
            return false;
        }

        let mut repo_count = 0;

        for op in &self.ops {
            match op {
                Op::CreateRepo { name: _, update: _ } => {
                    repo_count += 1;
                },
                Op::InspectRepo => {
                    if repo_count == 0 {
                        return false;
                    }
                },
                Op::DeleteRepo { force: _, all: _ } => {
                    if repo_count == 0 {
                        return false;
                    }
                    repo_count -= 1;
                },
                _ => {}
            }
        }

        true
    }
}

#[derive(Arbitrary, Clone, Debug, PartialEq)]
enum Op {
    CreateRepo { name: RepoName, update: bool },
    InspectRepo,
    ListRepo,
    DeleteRepo { force: bool, all: bool },
}

impl Op {
    fn valid(&self) -> bool {
        match self {
            Op::CreateRepo { name, update: _ } => name.valid(),
            _ => true
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
struct RepoName {
    bytes: Vec<u8>
}

impl RepoName {
    fn valid(&self) -> bool {
        self.bytes.len() > 0
    }
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
    let mut repos = Vec::new();

    delete_all(&mut pps_client, &mut pfs_client).await.unwrap();

    for op in opts.ops.into_iter() {
        match op {
            Op::CreateRepo { name, update } => {
                let name = name.to_string();
                
                check(pfs_client.create_repo(pfs::CreateRepoRequest {
                    repo: Some(pfs::Repo { name: name.clone() }),
                    description: "".into(),
                    update: update,
                }).await);

                repos.push(name);
            },

            Op::InspectRepo => {
                check(pfs_client.inspect_repo(pfs::InspectRepoRequest {
                    repo: Some(pfs::Repo { name: repos.last().unwrap().clone() }),
                }).await);
            },
            Op::ListRepo => {
                check(pfs_client.list_repo(pfs::ListRepoRequest {}).await);
            },

            Op::DeleteRepo { force, all } => {
                check(pfs_client.delete_repo(pfs::DeleteRepoRequest {
                    repo: Some(pfs::Repo { name: repos.pop().unwrap() }),
                    force,
                    all
                }).await);
            }
        }
    }

    delete_all(&mut pps_client, &mut pfs_client).await.unwrap();
}

fuzz_target!(|opts: Options| {
    if !opts.valid() {
        return;
    }
    Runtime::new().unwrap().block_on(run(opts));
});
