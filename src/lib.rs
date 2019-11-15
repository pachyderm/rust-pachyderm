extern crate bytes;
extern crate prost;
extern crate prost_types;

pub mod admin {
    include!(concat!(env!("OUT_DIR"), "/admin.rs"));
}

pub mod auth {
    include!(concat!(env!("OUT_DIR"), "/auth.rs"));
}

pub mod auth_1_7 {
    include!(concat!(env!("OUT_DIR"), "/auth_1_7.rs"));
}

pub mod config {
    include!(concat!(env!("OUT_DIR"), "/config.rs"));
}

pub mod debug {
    include!(concat!(env!("OUT_DIR"), "/debug.rs"));
}

pub mod deploy {
    include!(concat!(env!("OUT_DIR"), "/deploy.rs"));
}

pub mod enterprise {
    include!(concat!(env!("OUT_DIR"), "/enterprise.rs"));
}

pub mod enterprise_1_7 {
    include!(concat!(env!("OUT_DIR"), "/enterprise_1_7.rs"));
}

pub mod hashtree_1_7 {
    include!(concat!(env!("OUT_DIR"), "/hashtree_1_7.rs"));
}

pub mod health {
    include!(concat!(env!("OUT_DIR"), "/health.rs"));
}

pub mod pfs {
    include!(concat!(env!("OUT_DIR"), "/pfs.rs"));
}

pub mod pfs_1_7 {
    include!(concat!(env!("OUT_DIR"), "/pfs_1_7.rs"));
}

pub mod pps {
    include!(concat!(env!("OUT_DIR"), "/pps.rs"));
}

pub mod pps_1_7 {
    include!(concat!(env!("OUT_DIR"), "/pps_1_7.rs"));
}

pub mod shard {
    include!(concat!(env!("OUT_DIR"), "/shard.rs"));
}

pub mod versionpb {
    include!(concat!(env!("OUT_DIR"), "/versionpb.rs"));
}
