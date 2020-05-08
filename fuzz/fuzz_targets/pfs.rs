#![no_main]

use std::env;
use std::fmt;

use pachyderm::pfs;
use pachyderm::pfs::api_client::ApiClient as PfsClient;
use pachyderm::pps::api_client::ApiClient as PpsClient;

use libfuzzer_sys::fuzz_target;
use arbitrary::{Arbitrary, Unstructured};
use tokio::runtime::Runtime;
use tonic::transport::Channel;
use tonic::{Code, Status};
use futures::stream;

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
            },
            Code::Unknown => {
                if !err.message().contains("as it already exists") {
                    panic!("unexpected error: {}", err);
                }
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
        let mut branch_count = 0;
        let mut open_commit_count = 0;
        let mut file_count = 0;

        for op in &self.ops {
            match op {
                Op::CreateRepo { name, update: _ } => {
                    if !name.valid() {
                        return false;
                    }
                    repo_count += 1;
                },
                Op::InspectRepo => {
                    if repo_count == 0 {
                        return false;
                    }
                },
                Op::DeleteRepo { force: _, all } => {
                    if *all {
                        repo_count = 0;
                    } else {
                        if repo_count == 0 {
                            return false;
                        }
                        repo_count -= 1;
                    }

                    branch_count = 0;
                    open_commit_count = 0;
                    file_count = 0;
                },

                Op::StartCommit => {
                    if branch_count == 0 {
                        return false;
                    }
                    branch_count -= 1;
                    open_commit_count += 1;
                },
                Op::FinishCommit => {
                    if open_commit_count == 0 {
                        return false;
                    }
                    open_commit_count -= 1;
                },
                Op::InspectCommit { block_state: _, open } => {
                    if (*open && open_commit_count == 0) || (!open && branch_count == 0) {
                        return false;
                    }
                },
                Op::ListCommit { number: _, reverse: _ } => {
                    if repo_count == 0 {
                        return false;
                    }
                },
                Op::DeleteCommit { open } => {
                    if *open {
                        if open_commit_count == 0 {
                            return false;
                        }
                        open_commit_count -= 1;
                    } else {
                        if branch_count == 0 {
                            return false;
                        }
                        branch_count -= 1;
                    }
                    file_count = 0;
                },
                Op::FlushCommit => {
                    if branch_count == 0 {
                        return false;
                    }
                },

                Op::CreateBranch { name, new } => {
                    if !name.valid() || repo_count == 0 || (!new && branch_count == 0) {
                        return false;
                    }
                    branch_count += 1;
                },
                Op::InspectBranch => {
                    if branch_count == 0 {
                        return false;
                    }
                },
                Op::ListBranch { reverse: _ } => {
                    if repo_count == 0 {
                        return false;
                    }
                }
                Op::DeleteBranch { force: _ } => {
                    if branch_count == 0 {
                        return false;
                    }
                    file_count = 0;
                },

                Op::PutFile { open, path: _, bytes: _, delimiter: _, target_file_datums: _, target_file_bytes: _, header_records: _, overwrite_index: _ } => {
                    if *open {
                        if open_commit_count == 0 {
                            return false;
                        }
                    } else {
                        if branch_count == 0 {
                            return false;
                        }
                    }
                    file_count += 1;
                },
                Op::GetFile { offset_bytes: _, size_bytes: _ } => {
                    if file_count == 0 {
                        return false;
                    }
                },
                Op::InspectFile => {
                    if file_count == 0 {
                        return false;
                    }
                },
                Op::ListFile { full: _, history: _ } => {
                    if branch_count == 0 {
                        return false;
                    }
                },
                Op::WalkFile => {
                    if branch_count == 0 {
                        return false;
                    }
                },
                Op::GlobFile { pattern: _ } => {
                    if branch_count == 0 {
                        return false;
                    }
                },
                Op::DiffFile { shallow: _ } => {
                    if file_count < 2 {
                        return false;
                    }
                },
                Op::DeleteFile => {
                    if file_count == 0 {
                        return false;
                    }
                    file_count -= 1;
                },

                Op::DeleteAll => {
                    repo_count = 0;
                    branch_count = 0;
                    open_commit_count = 0;
                    file_count = 0;
                }
                _ => {}
            }
        }

        true
    }
}

// TODO: copy file
#[derive(Arbitrary, Clone, Debug, PartialEq)]
enum Op {
    CreateRepo { name: Name, update: bool },
    InspectRepo,
    ListRepo,
    DeleteRepo { force: bool, all: bool },

    StartCommit,
    FinishCommit,
    InspectCommit { block_state: BlockState, open: bool },
    ListCommit { number: u64, reverse: bool },
    DeleteCommit { open: bool },
    FlushCommit,

    PutFile {
        open: bool,
        path: String,
        bytes: Vec<u8>,
        delimiter: Delimiter,
        target_file_datums: i64,
        target_file_bytes: i64,
        header_records: i64,
        overwrite_index: Option<i64>
    },
    GetFile { offset_bytes: i64, size_bytes: i64 },
    InspectFile,
    ListFile { full: bool, history: i64 },
    WalkFile,
    GlobFile { pattern: String },
    DiffFile { shallow: bool },
    DeleteFile,

    CreateBranch { name: Name, new: bool },
    InspectBranch,
    ListBranch { reverse: bool },
    DeleteBranch { force: bool },

    DeleteAll,
}

impl Op {
    fn valid(&self) -> bool {
        match self {
            Op::CreateRepo { name, update: _ } => name.valid(),
            _ => true
        }
    }
}

#[derive(Clone, PartialEq)]
struct Name {
    bytes: Vec<u8>
}

impl Name {
    fn valid(&self) -> bool {
        self.bytes.len() > 0
    }
}

impl ToString for Name {
    fn to_string(&self) -> String {
        // Quick and dirty way to make valid repo names from arbitrary bytes.
        // As a small performance win, we could alternatively create a proper
        // `base64::Config` for this custom encoding.
        let s = base64::encode(&self.bytes);
        s.replace("+", "_").replace("/", "_").replace("=", "-")
    }
}

impl fmt::Debug for Name {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_string())
    }
}

impl Arbitrary for Name {
    fn arbitrary(u: &mut Unstructured<'_>) -> arbitrary::Result<Self> {
        let bytes = Vec::<u8>::arbitrary(u)?;
        Ok(Name { bytes })
    }

    fn size_hint(_: usize) -> (usize, Option<usize>) {
        (1, None)
    }
}

#[derive(Arbitrary, Clone, Debug, PartialEq)]
pub enum BlockState {
    Started,
    Ready,
    Finished
}

#[derive(Arbitrary, Clone, Debug, PartialEq)]
pub enum Delimiter {
    None,
    Json,
    Line,
    Sql,
    Csv
}

async fn run(opts: Options) {
    let pachd_address = env::var("PACHD_ADDRESS").expect("No `PACHD_ADDRESS` set");
    let mut pfs_client = PfsClient::connect(pachd_address.clone()).await.unwrap();
    let mut pps_client = PpsClient::connect(pachd_address.clone()).await.unwrap();

    let mut repos = Vec::new();
    let mut branches = Vec::<(String, String)>::new();
    let mut open_commits = Vec::<(String, String)>::new();
    let mut files = Vec::new();

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
                if all {
                    check(pfs_client.delete_repo(pfs::DeleteRepoRequest {
                        repo: None,
                        force,
                        all
                    }).await);

                    repos = Vec::new();
                } else {
                    check(pfs_client.delete_repo(pfs::DeleteRepoRequest {
                        repo: Some(pfs::Repo { name: repos.pop().unwrap() }),
                        force,
                        all
                    }).await);
                }

                branches = Vec::new();
                open_commits = Vec::new();
                files = Vec::new();
            },

            Op::StartCommit => {
                let (last_repo, last_branch) = branches.pop().unwrap();

                check(pfs_client.start_commit(pfs::StartCommitRequest {
                    parent: Some(pfs::Commit {
                        repo: Some(pfs::Repo { name: last_repo.clone() }),
                        id: last_branch.clone()
                    }),
                    description: "".to_string(),
                    branch: "".to_string(),
                    provenance: Vec::default(),
                }).await);

                open_commits.push((last_repo, last_branch));
            },
            Op::FinishCommit => {
                let (last_repo, last_branch) = open_commits.pop().unwrap();

                let mut req = pfs::FinishCommitRequest::default();
                req.commit = Some(pfs::Commit {
                    repo: Some(pfs::Repo { name: last_repo.clone() }),
                    id: last_branch.clone()
                });

                check(pfs_client.finish_commit(req).await);

                branches.push((last_repo, last_branch));
            },
            Op::InspectCommit { block_state, open } => {
                let (last_repo, last_branch) = if open {
                    open_commits.last().unwrap().clone()
                } else {
                    branches.last().unwrap().clone()
                };

                check(pfs_client.inspect_commit(pfs::InspectCommitRequest {
                    commit: Some(pfs::Commit {
                        repo: Some(pfs::Repo { name: last_repo }),
                        id: last_branch,
                    }),
                    block_state: match block_state {
                        BlockState::Started => pfs::CommitState::Started.into(),
                        BlockState::Ready => pfs::CommitState::Ready.into(),
                        BlockState::Finished => pfs::CommitState::Finished.into(),
                    }
                }).await);
            },
            Op::ListCommit { number, reverse } => {
                let last_repo = repos.last().unwrap().clone();

                check(pfs_client.list_commit(pfs::ListCommitRequest {
                    repo: Some(pfs::Repo { name: last_repo }),
                    from: None,
                    to: None,
                    number,
                    reverse,
                }).await);
            },
            Op::DeleteCommit { open } => {
                let (last_repo, last_branch) = if open {
                    open_commits.pop().unwrap()
                } else {
                    branches.pop().unwrap()
                };

                check(pfs_client.delete_commit(pfs::DeleteCommitRequest {
                    commit: Some(pfs::Commit {
                        repo: Some(pfs::Repo { name: last_repo }),
                        id: last_branch,
                    })
                }).await);

                files = Vec::new();
            },
            Op::FlushCommit => {
                let (last_repo, last_branch) = branches.last().unwrap().clone();

                check(pfs_client.flush_commit(pfs::FlushCommitRequest {
                    commits: vec![pfs::Commit {
                        repo: Some(pfs::Repo { name: last_repo }),
                        id: last_branch,
                    }],
                    to_repos: Vec::default()
                }).await);
            },

            Op::PutFile { open, path, bytes, delimiter, target_file_datums, target_file_bytes, header_records, overwrite_index } => {
                let (last_repo, last_branch) = if open {
                    open_commits.last().unwrap().clone()
                } else {
                    branches.last().unwrap().clone()
                };

                let req = pfs::PutFileRequest {
                    file: Some(pfs::File {
                        commit: Some(pfs::Commit {
                            repo: Some(pfs::Repo { name: last_repo.clone() }),
                            id: last_branch.clone(),
                        }),
                        path: path.clone(),
                    }),
                    value: bytes,
                    url: "".to_string(),
                    recursive: false,
                    delimiter: match delimiter {
                        Delimiter::None => pfs::Delimiter::None.into(),
                        Delimiter::Json => pfs::Delimiter::Json.into(),
                        Delimiter::Line => pfs::Delimiter::Line.into(),
                        Delimiter::Sql => pfs::Delimiter::Sql.into(),
                        Delimiter::Csv => pfs::Delimiter::Csv.into(),
                    },
                    target_file_datums,
                    target_file_bytes,
                    header_records,
                    overwrite_index: overwrite_index.map(|index| pfs::OverwriteIndex { index }),
                };

                check(pfs_client.put_file(stream::iter(vec![req])).await);
                files.push((last_repo, last_branch, path));
            },
            Op::GetFile { offset_bytes, size_bytes } => {
                let (last_repo, last_branch, last_path) = files.last().unwrap().clone();

                check(pfs_client.get_file(pfs::GetFileRequest {
                    file: Some(pfs::File {
                        commit: Some(pfs::Commit {
                            repo: Some(pfs::Repo { name: last_repo }),
                            id: last_branch,
                        }),
                        path: last_path,
                    }),
                    offset_bytes,
                    size_bytes,
                }).await);
            },
            Op::InspectFile => {
                let (last_repo, last_branch, last_path) = files.last().unwrap().clone();

                check(pfs_client.inspect_file(pfs::InspectFileRequest {
                    file: Some(pfs::File {
                        commit: Some(pfs::Commit {
                            repo: Some(pfs::Repo { name: last_repo }),
                            id: last_branch,
                        }),
                        path: last_path,
                    }),
                }).await);
            },
            Op::ListFile { full, history } => {
                let (last_repo, last_branch) = branches.last().unwrap().clone();

                check(pfs_client.list_file(pfs::ListFileRequest {
                    file: Some(pfs::File {
                        commit: Some(pfs::Commit {
                            repo: Some(pfs::Repo { name: last_repo }),
                            id: last_branch,
                        }),
                        path: "".to_string(),
                    }),
                    full,
                    history,
                }).await);
            },
            Op::WalkFile => {
                let (last_repo, last_branch) = branches.last().unwrap().clone();

                check(pfs_client.walk_file(pfs::WalkFileRequest {
                    file: Some(pfs::File {
                        commit: Some(pfs::Commit {
                            repo: Some(pfs::Repo { name: last_repo }),
                            id: last_branch,
                        }),
                        path: "".to_string(),
                    }),
                }).await);
            },
            Op::GlobFile { pattern } => {
                let (last_repo, last_branch) = branches.last().unwrap().clone();

                check(pfs_client.glob_file(pfs::GlobFileRequest {
                    commit: Some(pfs::Commit {
                        repo: Some(pfs::Repo { name: last_repo }),
                        id: last_branch,
                    }),
                    pattern,
                }).await);
            },
            Op::DiffFile { shallow } => {
                let (first_repo, first_branch, first_path) = files[files.len()-2].clone();
                let (second_repo, second_branch, second_path) = files[files.len()-1].clone();

                check(pfs_client.diff_file(pfs::DiffFileRequest {
                    old_file: Some(pfs::File {
                        commit: Some(pfs::Commit {
                            repo: Some(pfs::Repo { name: first_repo }),
                            id: first_branch,
                        }),
                        path: first_path,
                    }),
                    new_file: Some(pfs::File {
                        commit: Some(pfs::Commit {
                            repo: Some(pfs::Repo { name: second_repo }),
                            id: second_branch,
                        }),
                        path: second_path,
                    }),
                    shallow,
                }).await);
            },
            Op::DeleteFile => {
                let (last_repo, last_branch, last_path) = files.pop().unwrap();

                check(pfs_client.delete_file(pfs::DeleteFileRequest {
                    file: Some(pfs::File {
                        commit: Some(pfs::Commit {
                            repo: Some(pfs::Repo { name: last_repo }),
                            id: last_branch,
                        }),
                        path: last_path,
                    }),
                }).await);
            },

            Op::CreateBranch { name, new } => {
                let last_repo = repos.last().unwrap().clone();

                check(pfs_client.create_branch(pfs::CreateBranchRequest {
                    head: if new {
                        None
                    } else {
                        let (last_repo, last_branch) = branches.last().unwrap().clone();
                        Some(pfs::Commit {
                            repo: Some(pfs::Repo { name: last_repo }),
                            id: last_branch,
                        })
                    },
                    branch: Some(pfs::Branch {
                        repo: Some(pfs::Repo { name: last_repo.clone() }),
                        name: name.to_string(),
                    }),
                    s_branch: "".to_string(),
                    provenance: Vec::default(),
                }).await);

                branches.push((last_repo, name.to_string()));
            },
            Op::InspectBranch => {
                let (last_repo, last_branch) = branches.last().unwrap().clone();

                check(pfs_client.inspect_branch(pfs::InspectBranchRequest {
                    branch: Some(pfs::Branch {
                        repo: Some(pfs::Repo { name: last_repo }),
                        name: last_branch,
                    })
                }).await);
            },
            Op::ListBranch { reverse } => {
                check(pfs_client.list_branch(pfs::ListBranchRequest {
                    repo: Some(pfs::Repo { name: repos.last().unwrap().clone() }),
                    reverse: reverse
                }).await);
            },
            Op::DeleteBranch { force } => {
                let (last_repo, last_branch) = branches.pop().unwrap();

                check(pfs_client.delete_branch(pfs::DeleteBranchRequest {
                    branch: Some(pfs::Branch {
                        repo: Some(pfs::Repo { name: last_repo }),
                        name: last_branch,
                    }),
                    force
                }).await);
            },

            Op::DeleteAll => {
                pfs_client.delete_all(()).await.unwrap();
                repos = Vec::new();
                branches = Vec::new();
                open_commits = Vec::new();
                files = Vec::new();
            }
        }
    }

    // ensure it passes fsck
    pfs_client.fsck(pfs::FsckRequest { fix: false }).await.unwrap();

    delete_all(&mut pps_client, &mut pfs_client).await.unwrap();
}

fuzz_target!(|opts: Options| {
    if !opts.valid() {
        return;
    }
    Runtime::new().unwrap().block_on(run(opts));
});
