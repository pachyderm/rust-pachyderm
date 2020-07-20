extern crate bytes;
extern crate prost;
extern crate prost_types;
extern crate tonic;

pub mod admin {
    tonic::include_proto!("admin");
}

pub mod auth {
    tonic::include_proto!("auth");
}

mod auth_1_7 {
    tonic::include_proto!("auth_1_7");
}

mod auth_1_8 {
    tonic::include_proto!("auth_1_8");
}

mod auth_1_9 {
    tonic::include_proto!("auth_1_9");
}

mod auth_1_10 {
    tonic::include_proto!("auth_1_10");
}

pub mod debug {
    tonic::include_proto!("debug");
}

pub mod enterprise {
    tonic::include_proto!("enterprise");
}

pub mod health {
    tonic::include_proto!("health");
}

pub mod pfs {
    tonic::include_proto!("pfs");
}

mod pfs_1_7 {
    tonic::include_proto!("pfs_1_7");
}

mod pfs_1_8 {
    tonic::include_proto!("pfs_1_8");
}

mod pfs_1_9 {
    tonic::include_proto!("pfs_1_9");
}

mod pfs_1_10 {
    tonic::include_proto!("pfs_1_10");
}

pub mod pps {
    tonic::include_proto!("pps");
}

mod pps_1_7 {
    tonic::include_proto!("pps_1_7");
}

mod pps_1_8 {
    tonic::include_proto!("pps_1_8");
}

mod pps_1_9 {
    tonic::include_proto!("pps_1_9");
}

mod pps_1_10 {
    tonic::include_proto!("pps_1_10");
}

pub mod transaction {
    tonic::include_proto!("transaction");
}

pub mod version {
    tonic::include_proto!("versionpb");
}
