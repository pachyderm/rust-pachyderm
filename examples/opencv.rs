//! This is the rust equivalent of the canonical OpenCV example for Pachyderm:
//! https://github.com/pachyderm/pachyderm/tree/master/examples/opencv
//! See also the equivalent example in the python library:
//! https://github.com/pachyderm/python-pachyderm/blob/master/examples/opencv/opencv.py

extern crate futures;
extern crate pachyderm;
extern crate tokio;
extern crate tonic;

use std::env;
use std::error::Error;

use pachyderm::pfs::{
    api_client::ApiClient as PfsClient, Commit, CreateRepoRequest, File, FinishCommitRequest, PutFileRequest, Repo,
    StartCommitRequest,
};
use pachyderm::pps::{api_client::ApiClient as PpsClient, CreatePipelineRequest, Input, PfsInput, Pipeline, Transform};

use futures::stream;
use tonic::transport::Channel;
use tonic::Request;

fn create_pfs_input(glob: &str, repo: &str) -> Input {
    let mut pfs_input = PfsInput::default();
    pfs_input.glob = glob.into();
    pfs_input.repo = repo.into();

    let mut input = Input::default();
    input.pfs = Some(pfs_input);

    input
}

async fn create_pipeline(
    pps_client: &mut PpsClient<Channel>,
    name: &str,
    transform_image: &str,
    transform_cmd: Vec<&str>,
    transform_stdin: Option<&str>,
    input: Input,
) -> Result<(), Box<dyn Error>> {
    let mut request = CreatePipelineRequest::default();
    request.pipeline = Some(Pipeline { name: name.into() });

    let mut transform = Transform::default();
    transform.image = transform_image.into();
    transform.cmd = transform_cmd.into_iter().map(|i| i.into()).collect();
    if let Some(stdin) = transform_stdin {
        transform.stdin = vec![stdin.into()];
    }
    request.transform = Some(transform);
    request.input = Some(input);

    pps_client.create_pipeline(Request::new(request)).await?;
    Ok(())
}

async fn put_file_url(
    pfs_client: &mut PfsClient<Channel>,
    commit: Commit,
    path: &str,
    url: &str,
) -> Result<(), Box<dyn Error>> {
    let mut request = PutFileRequest::default();

    request.file = Some(File {
        commit: Some(commit),
        path: path.into(),
    });
    request.url = url.into();

    let request_stream = stream::iter(vec![request]);
    pfs_client.put_file(request_stream).await?;
    Ok(())
}

async fn create_images_repo(pfs_client: &mut PfsClient<Channel>) -> Result<(), Box<dyn Error>> {
    let mut request = CreateRepoRequest::default();
    request.repo = Some(Repo { name: "images".into() });
    pfs_client.create_repo(Request::new(request)).await?;
    Ok(())
}

async fn create_edges_pipeline(pps_client: &mut PpsClient<Channel>) -> Result<(), Box<dyn Error>> {
    let input = create_pfs_input("/*", "images");
    create_pipeline(
        pps_client,
        "edges",
        "pachyderm/opencv",
        vec!["python3", "edges.py"],
        None,
        input,
    )
    .await?;
    Ok(())
}

async fn create_montage_pipeline(pps_client: &mut PpsClient<Channel>) -> Result<(), Box<dyn Error>> {
    let mut input = Input::default();
    input.cross = vec![create_pfs_input("/", "images"), create_pfs_input("/", "edges")];

    create_pipeline(
        pps_client,
        "montage",
        "v4tech/imagemagick",
        vec!["sh"],
        Some("montage -shadow -background SkyBlue -geometry 300x300+2+2 $(find /pfs -type f | sort) /pfs/out/montage.png"),
        input
    ).await?;

    Ok(())
}

async fn put_example_images(pfs_client: &mut PfsClient<Channel>) -> Result<(), Box<dyn Error>> {
    // put a file from URL
    put_file_url(
        pfs_client,
        Commit {
            repo: Some(Repo { name: "images".into() }),
            id: "master".into(),
        },
        "46Q8nDz.jpg",
        "http://imgur.com/46Q8nDz.jpg",
    )
    .await?;

    // put multiple files from URLs in a single a commit
    let mut parent = Commit::default();
    parent.repo = Some(Repo { name: "images".into() });

    let mut commit = StartCommitRequest::default();
    commit.parent = Some(parent);
    commit.branch = "master".into();
    let commit = pfs_client.start_commit(Request::new(commit)).await?.into_inner();

    put_file_url(
        pfs_client,
        commit.clone(),
        "g2QnNqa.jpg",
        "http://imgur.com/g2QnNqa.jpg",
    )
    .await?;
    put_file_url(
        pfs_client,
        commit.clone(),
        "8MN9Kg0.jpg",
        "http://imgur.com/8MN9Kg0.jpg",
    )
    .await?;

    let mut request = FinishCommitRequest::default();
    request.commit = Some(commit);
    pfs_client.finish_commit(request).await?;

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let mut args = env::args().collect::<Vec<String>>();
    let pachd_address = if args.len() > 1 {
        args.pop().unwrap()
    } else {
        "grpc://localhost:30650".into()
    };

    let mut pfs_client = PfsClient::connect(pachd_address.clone()).await?;
    let mut pps_client = PpsClient::connect(pachd_address.clone()).await?;
    create_images_repo(&mut pfs_client).await?;
    create_edges_pipeline(&mut pps_client).await?;
    create_montage_pipeline(&mut pps_client).await?;
    put_example_images(&mut pfs_client).await?;
    Ok(())
}
