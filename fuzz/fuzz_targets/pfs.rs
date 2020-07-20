#![no_main]

//! Fuzzes PFS by running a fuzz-generated list of operations. Given the
//! variety of operations available in PFS, the state space is _huge_, so we
//! trim it out substantially by constructing a local representation of
//! PFS-stored content, treated largely as a stack. The trade-off is that this
//! fuzzer can't test a number of PFS situations, but this is hopefully an
//! 80-20 sort of deal.

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

/// Deletes everything in the cluster
async fn delete_all(pps_client: &mut PpsClient<Channel>, pfs_client: &mut PfsClient<Channel>) -> Result<(), Status> {
    pps_client.delete_all(()).await?;
    pfs_client.delete_all(()).await?;
    Ok(())
}

/// Checks if an unexpected error occurs and panics if so
fn check<T>(result: Result<T, Status>) {
    if let Err(err) = result {
        match err.code() {
            Code::Unknown | Code::Cancelled | Code::DeadlineExceeded | Code::NotFound | Code::AlreadyExists | Code::PermissionDenied | Code::ResourceExhausted | Code::FailedPrecondition | Code::Aborted | Code::OutOfRange | Code::Unimplemented | Code::Internal | Code::Unavailable | Code::DataLoss | Code::Unauthenticated => {
                panic!("unexpected error: {:?}\n{}", err.code(), err);
            },
            _ => {
                println!("{}", err);
            }
        }
    }
}

/// Represents a fuzz run
#[derive(Clone, Debug, PartialEq)]
struct Options {
    /// A list of operations to run on PFS
    ops: Vec<Op>,
}

impl Arbitrary for Options {
    fn arbitrary(u: &mut Unstructured<'_>) -> arbitrary::Result<Self> {
        let ops = Vec::<Op>::arbitrary(u)?;

        // We need some list of operations to run, and cargo-fuzz generates
        // quite a few empty lists even when `size_hint` is specified.
        if ops.is_empty() {
            return Err(arbitrary::Error::NotEnoughData);
        }

        let mut state = State::default();

        // Verify that the operations should pass
        for op in &ops {
            match op {
                Op::CreateRepo { name } => {
                    if state.repos.iter().any(|repo| &repo.name == name) {
                        return Err(arbitrary::Error::IncorrectFormat);
                    }

                    state.push_repo(name.clone());
                },
                Op::InspectRepo => {
                    state.repo()?;
                },
                Op::DeleteRepo { force: _, all } => {
                    if *all {
                        state = State::default();
                    } else {
                        state.pop_repo()?;
                    }
                },

                Op::StartCommit => {
                    let mut branch = state.closed_branch()?;
                    branch.open = true;
                },
                Op::FinishCommit => {
                    let mut branch = state.open_branch()?;
                    branch.open = false;
                },
                Op::InspectCommit { block_state: _ } => {
                    state.branch()?;
                },
                Op::ListCommit { number: _, reverse: _ } => {
                    state.repo()?;
                },
                Op::DeleteCommit => {
                    let branch = state.pop_branch()?;

                    // ensure branch has a head
                    if !branch.open && branch.files.is_empty() {
                        return Err(arbitrary::Error::IncorrectFormat);
                    }
                },
                Op::FlushCommit => {
                    state.closed_branch()?;
                },

                Op::PutFile { name, bytes: _, delimiter: _, target_file_datums: _, target_file_bytes: _, header_records: _, overwrite_index: _ } => {
                    let mut branch = state.branch()?;
                    branch.push_file(name.clone());
                },
                Op::GetFile { offset_bytes: _, size_bytes: _ } => {
                    state.file()?;
                },
                Op::InspectFile => {
                    state.file()?;
                },
                Op::ListFile { full: _, history: _ } => {
                    state.branch()?;
                },
                Op::WalkFile => {
                    state.branch()?;
                },
                Op::GlobFile { pattern: _ } => {
                    state.branch()?;
                },
                Op::DiffFile { shallow: _ } => {
                    state.pop_file()?;
                    state.pop_file()?;
                },
                Op::DeleteFile => {
                    state.pop_file()?;
                },

                Op::CreateBranch { name, new } => {
                    let mut repo = state.repo()?;
                    if !new {
                        state.closed_branch()?;
                    }
                    repo.push_branch(name.clone());
                },
                Op::InspectBranch => {
                    state.branch()?;
                },
                Op::ListBranch { reverse: _ } => {
                    state.repo()?;
                }
                Op::DeleteBranch { force: _ } => {
                    state.pop_branch()?;
                },

                Op::DeleteAll => {
                    state = State::default();
                }
                _ => {}
            }
        }

        Ok(Options { ops })
    }
}

// TODO: copy file
/// Represents an operation to run
#[derive(Arbitrary, Clone, Debug, PartialEq)]
enum Op {
    CreateRepo { name: Name },
    InspectRepo,
    ListRepo,
    DeleteRepo { force: bool, all: bool },

    StartCommit,
    FinishCommit,
    InspectCommit { block_state: BlockState },
    ListCommit { number: u64, reverse: bool },
    DeleteCommit,
    FlushCommit,

    PutFile {
        name: Name,
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

/// Represents a fuzz-generated repo, branch, or file name
#[derive(Clone, PartialEq)]
struct Name {
    bytes: Vec<u8>
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
        if bytes.is_empty() {
            Err(arbitrary::Error::NotEnoughData)
        } else {
            Ok(Name { bytes })
        }
    }

    fn size_hint(_: usize) -> (usize, Option<usize>) {
        (1, None)
    }
}

/// A local representation of the PFS state. Uses stacks extensively to reduce
/// the amount of random data that needs to be generated and validated from the fuzzer.
#[derive(Default, Arbitrary, Clone, Debug, PartialEq)]
pub struct State {
    /// List of repos expected to be in PFS
    repos: Vec<Repo>
}

impl State {
    /// Gets the top repo
    fn repo(&self) -> Result<Repo, arbitrary::Error> {
        self.repos.last().cloned().ok_or(arbitrary::Error::IncorrectFormat)
    }

    /// Adds a new repo
    fn push_repo(&mut self, name: Name) {
        self.repos.push(Repo { name, branches: Vec::new() });
    }

    /// Removes and returns the top repo
    fn pop_repo(&mut self) -> Result<Repo, arbitrary::Error> {
        self.repos.pop().ok_or(arbitrary::Error::IncorrectFormat)
    }

    /// Gets the top branch
    fn branch(&self) -> Result<Branch, arbitrary::Error> {
        self.repos.iter().flat_map(|repo| repo.branches.iter()).last().cloned().ok_or(arbitrary::Error::IncorrectFormat)
    }

    /// Gets the top branch with an open commit
    fn open_branch(&self) -> Result<Branch, arbitrary::Error> {
        self.repos.iter().flat_map(|repo| repo.branches.iter()).filter(|branch| branch.open).last().cloned().ok_or(arbitrary::Error::IncorrectFormat)
    }

    /// Gets the top branch without an open commit
    fn closed_branch(&self) -> Result<Branch, arbitrary::Error> {
        self.repos.iter().flat_map(|repo| repo.branches.iter()).filter(|branch| !branch.open).last().cloned().ok_or(arbitrary::Error::IncorrectFormat)
    }

    /// Removes and returns the top branch
    fn pop_branch(&mut self) -> Result<Branch, arbitrary::Error> {
        for repo in self.repos.iter_mut().rev() {
            if let Some(branch) = repo.branches.pop() {
                return Ok(branch);
            }
        }

        return Err(arbitrary::Error::IncorrectFormat);
    }

    /// Gets the top file
    fn file(&self) -> Result<File, arbitrary::Error> {
        self.repos.iter().flat_map(|repo| repo.branches.iter()).flat_map(|branch| branch.files.iter()).last().cloned().ok_or(arbitrary::Error::IncorrectFormat)
    }

    /// Removes and returns the top file
    fn pop_file(&mut self) -> Result<File, arbitrary::Error> {
        for repo in self.repos.iter_mut().rev() {
            for branch in repo.branches.iter_mut().rev() {
                if let Some(file) = branch.files.pop() {
                    return Ok(file);
                }
            }
        }

        return Err(arbitrary::Error::IncorrectFormat);
    }
}

/// A local representation of a PFS repo
#[derive(Arbitrary, Clone, Debug, PartialEq)]
pub struct Repo {
    /// The name of the repo
    name: Name,
    /// The repo's branches
    branches: Vec<Branch>
}

impl Repo {
    /// Adds a branch to the repo
    fn push_branch(&mut self, name: Name) {
        self.branches.push(Branch { name, repo_name: self.name.clone(), open: false, files: Vec::default() });
    }
}

/// A local representation of a PFS branch
#[derive(Arbitrary, Clone, Debug, PartialEq)]
pub struct Branch {
    /// The name of the repo this branch is in
    repo_name: Name,
    /// The name of the branch
    name: Name,
    /// Whether the branch has an open commit
    open: bool,
    /// The files in the branch's HEAD commit
    files: Vec<File>
}

impl Branch {
    /// Adds a file to the branch
    fn push_file(&mut self, name: Name) {
        self.files.push(File { name, repo_name: self.repo_name.clone(), branch_name: self.name.clone() });
    }
}

/// A local representation of a PFS file
#[derive(Arbitrary, Clone, Debug, PartialEq)]
pub struct File {
    /// The repo that this file is in
    repo_name: Name,
    /// The branch that this file is in
    branch_name: Name,
    /// The name of the file
    name: Name
}

/// Fuzz-generatable block state options
#[derive(Arbitrary, Clone, Debug, PartialEq)]
pub enum BlockState {
    Started,
    Ready,
    Finished
}

/// Fuzz-generatable delimiter options
#[derive(Arbitrary, Clone, Debug, PartialEq)]
pub enum Delimiter {
    None,
    Json,
    Line,
    Sql,
    Csv
}

/// Does a fuzz run
async fn run(opts: Options) {
    let pachd_address = env::var("PACHD_ADDRESS").expect("No `PACHD_ADDRESS` set");
    let mut pfs_client = PfsClient::connect(pachd_address.clone()).await.unwrap();
    let mut pps_client = PpsClient::connect(pachd_address.clone()).await.unwrap();

    let mut state = State::default();

    delete_all(&mut pps_client, &mut pfs_client).await.unwrap();

    // Run all of the operations. There are a lot of `.unwrap()`'s because the
    // operations should already have been validated in the custom `Arbitrary`
    // implementation.
    for op in opts.ops.into_iter() {
        match op {
            Op::CreateRepo { name } => {
                check(pfs_client.create_repo(pfs::CreateRepoRequest {
                    repo: Some(pfs::Repo { name: name.to_string() }),
                    description: "".into(),
                    update: false,
                }).await);

                state.push_repo(name);
            },

            Op::InspectRepo => {
                check(pfs_client.inspect_repo(pfs::InspectRepoRequest {
                    repo: Some(pfs::Repo { name: state.repo().unwrap().name.to_string() }),
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

                    state = State::default();
                } else {
                    check(pfs_client.delete_repo(pfs::DeleteRepoRequest {
                        repo: Some(pfs::Repo { name: state.pop_repo().unwrap().name.to_string() }),
                        force,
                        all
                    }).await);
                }
            },

            Op::StartCommit => {
                let mut branch = state.closed_branch().unwrap();

                check(pfs_client.start_commit(pfs::StartCommitRequest {
                    parent: Some(pfs::Commit {
                        repo: Some(pfs::Repo { name: branch.repo_name.to_string() }),
                        id: branch.name.to_string()
                    }),
                    description: "".to_string(),
                    branch: "".to_string(),
                    provenance: Vec::default(),
                }).await);

                branch.open = true;
            },
            Op::FinishCommit => {
                let mut branch = state.open_branch().unwrap();

                let mut req = pfs::FinishCommitRequest::default();
                req.commit = Some(pfs::Commit {
                    repo: Some(pfs::Repo { name: branch.repo_name.to_string() }),
                    id: branch.name.to_string()
                });

                check(pfs_client.finish_commit(req).await);

                branch.open = false;
            },
            Op::InspectCommit { block_state } => {
                let branch = state.branch().unwrap();

                check(pfs_client.inspect_commit(pfs::InspectCommitRequest {
                    commit: Some(pfs::Commit {
                        repo: Some(pfs::Repo { name: branch.repo_name.to_string() }),
                        id: branch.name.to_string(),
                    }),
                    block_state: match block_state {
                        BlockState::Started => pfs::CommitState::Started.into(),
                        BlockState::Ready => pfs::CommitState::Ready.into(),
                        BlockState::Finished => pfs::CommitState::Finished.into(),
                    }
                }).await);
            },
            Op::ListCommit { number, reverse } => {
                let repo = state.repo().unwrap();

                check(pfs_client.list_commit(pfs::ListCommitRequest {
                    repo: Some(pfs::Repo { name: repo.name.to_string() }),
                    from: None,
                    to: None,
                    number,
                    reverse,
                }).await);
            },
            Op::DeleteCommit => {
                let branch = state.pop_branch().unwrap();

                check(pfs_client.delete_commit(pfs::DeleteCommitRequest {
                    commit: Some(pfs::Commit {
                        repo: Some(pfs::Repo { name: branch.repo_name.to_string() }),
                        id: branch.name.to_string(),
                    })
                }).await);
            },
            Op::FlushCommit => {
                let branch = state.closed_branch().unwrap();

                check(pfs_client.flush_commit(pfs::FlushCommitRequest {
                    commits: vec![pfs::Commit {
                        repo: Some(pfs::Repo { name: branch.repo_name.to_string() }),
                        id: branch.name.to_string(),
                    }],
                    to_repos: Vec::default()
                }).await);
            },

            Op::PutFile { name, bytes, delimiter, target_file_datums, target_file_bytes, header_records, overwrite_index } => {
                let mut branch = state.branch().unwrap();

                let req = pfs::PutFileRequest {
                    file: Some(pfs::File {
                        commit: Some(pfs::Commit {
                            repo: Some(pfs::Repo { name: branch.repo_name.to_string() }),
                            id: branch.name.to_string(),
                        }),
                        path: name.to_string(),
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

                branch.push_file(name);
            },
            Op::GetFile { offset_bytes, size_bytes } => {
                let file = state.file().unwrap();

                check(pfs_client.get_file(pfs::GetFileRequest {
                    file: Some(pfs::File {
                        commit: Some(pfs::Commit {
                            repo: Some(pfs::Repo { name: file.repo_name.to_string() }),
                            id: file.branch_name.to_string(),
                        }),
                        path: file.name.to_string(),
                    }),
                    offset_bytes,
                    size_bytes,
                }).await);
            },
            Op::InspectFile => {
                let file = state.file().unwrap();

                check(pfs_client.inspect_file(pfs::InspectFileRequest {
                    file: Some(pfs::File {
                        commit: Some(pfs::Commit {
                            repo: Some(pfs::Repo { name: file.repo_name.to_string() }),
                            id: file.branch_name.to_string(),
                        }),
                        path: file.name.to_string(),
                    }),
                }).await);
            },
            Op::ListFile { full, history } => {
                let branch = state.branch().unwrap();

                check(pfs_client.list_file(pfs::ListFileRequest {
                    file: Some(pfs::File {
                        commit: Some(pfs::Commit {
                            repo: Some(pfs::Repo { name: branch.repo_name.to_string() }),
                            id: branch.name.to_string(),
                        }),
                        path: "".to_string(),
                    }),
                    full,
                    history,
                }).await);
            },
            Op::WalkFile => {
                let branch = state.branch().unwrap();

                check(pfs_client.walk_file(pfs::WalkFileRequest {
                    file: Some(pfs::File {
                        commit: Some(pfs::Commit {
                            repo: Some(pfs::Repo { name: branch.repo_name.to_string() }),
                            id: branch.name.to_string(),
                        }),
                        path: "".to_string(),
                    }),
                }).await);
            },
            Op::GlobFile { pattern } => {
                let branch = state.branch().unwrap();

                check(pfs_client.glob_file(pfs::GlobFileRequest {
                    commit: Some(pfs::Commit {
                        repo: Some(pfs::Repo { name: branch.repo_name.to_string() }),
                        id: branch.name.to_string(),
                    }),
                    pattern,
                }).await);
            },
            Op::DiffFile { shallow } => {
                let f1 = state.pop_file().unwrap();
                let f2 = state.pop_file().unwrap();

                check(pfs_client.diff_file(pfs::DiffFileRequest {
                    old_file: Some(pfs::File {
                        commit: Some(pfs::Commit {
                            repo: Some(pfs::Repo { name: f1.repo_name.to_string() }),
                            id: f1.branch_name.to_string(),
                        }),
                        path: f1.name.to_string(),
                    }),
                    new_file: Some(pfs::File {
                        commit: Some(pfs::Commit {
                            repo: Some(pfs::Repo { name: f2.repo_name.to_string() }),
                            id: f2.branch_name.to_string(),
                        }),
                        path: f2.name.to_string(),
                    }),
                    shallow,
                }).await);
            },
            Op::DeleteFile => {
                let file = state.pop_file().unwrap();

                check(pfs_client.delete_file(pfs::DeleteFileRequest {
                    file: Some(pfs::File {
                        commit: Some(pfs::Commit {
                            repo: Some(pfs::Repo { name: file.repo_name.to_string() }),
                            id: file.branch_name.to_string(),
                        }),
                        path: file.name.to_string(),
                    }),
                }).await);
            },

            Op::CreateBranch { name, new } => {
                let mut repo = state.repo().unwrap();

                check(pfs_client.create_branch(pfs::CreateBranchRequest {
                    head: if new {
                        None
                    } else {
                        let branch = state.closed_branch().unwrap();
                        Some(pfs::Commit {
                            repo: Some(pfs::Repo { name: branch.repo_name.to_string() }),
                            id: branch.name.to_string(),
                        })
                    },
                    branch: Some(pfs::Branch {
                        repo: Some(pfs::Repo { name: repo.name.to_string() }),
                        name: name.to_string(),
                    }),
                    s_branch: "".to_string(),
                    provenance: Vec::default(),
                }).await);

                repo.push_branch(name);
            },
            Op::InspectBranch => {
                let branch = state.branch().unwrap();

                check(pfs_client.inspect_branch(pfs::InspectBranchRequest {
                    branch: Some(pfs::Branch {
                        repo: Some(pfs::Repo { name: branch.repo_name.to_string() }),
                        name: branch.name.to_string(),
                    })
                }).await);
            },
            Op::ListBranch { reverse } => {
                let repo = state.repo().unwrap();

                check(pfs_client.list_branch(pfs::ListBranchRequest {
                    repo: Some(pfs::Repo { name: repo.name.to_string() }),
                    reverse: reverse
                }).await);
            },
            Op::DeleteBranch { force } => {
                let branch = state.pop_branch().unwrap();

                check(pfs_client.delete_branch(pfs::DeleteBranchRequest {
                    branch: Some(pfs::Branch {
                        repo: Some(pfs::Repo { name: branch.repo_name.to_string() }),
                        name: branch.name.to_string(),
                    }),
                    force
                }).await);
            },

            Op::DeleteAll => {
                pfs_client.delete_all(()).await.unwrap();
                state = State::default();
            }
        }
    }

    // ensure it passes fsck
    pfs_client.fsck(pfs::FsckRequest { fix: false }).await.unwrap();

    delete_all(&mut pps_client, &mut pfs_client).await.unwrap();
}

fuzz_target!(|opts: Options| {
    Runtime::new().unwrap().block_on(run(opts));
});
