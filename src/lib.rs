extern crate bytes;
extern crate prost;
extern crate prost_types;

pub mod admin {
    include!(concat!(env!("OUT_DIR"), "/admin.rs"));
}

pub mod auth {
    include!(concat!(env!("OUT_DIR"), "/auth.rs"));
}

mod auth_1_7 {
    include!(concat!(env!("OUT_DIR"), "/auth_1_7.rs"));
}

mod auth_1_8 {
    include!(concat!(env!("OUT_DIR"), "/auth_1_8.rs"));
}

pub mod debug {
    include!(concat!(env!("OUT_DIR"), "/debug.rs"));
}

pub mod enterprise {
    include!(concat!(env!("OUT_DIR"), "/enterprise.rs"));
}

pub mod health {
    include!(concat!(env!("OUT_DIR"), "/health.rs"));
}

pub mod pfs {
    include!(concat!(env!("OUT_DIR"), "/pfs.rs"));
}

mod pfs_1_7 {
    include!(concat!(env!("OUT_DIR"), "/pfs_1_7.rs"));
}

mod pfs_1_8 {
    include!(concat!(env!("OUT_DIR"), "/pfs_1_8.rs"));
}

pub mod pps {
    include!(concat!(env!("OUT_DIR"), "/pps.rs"));
}

mod pps_1_7 {
    include!(concat!(env!("OUT_DIR"), "/pps_1_7.rs"));
}

mod pps_1_8 {
    include!(concat!(env!("OUT_DIR"), "/pps_1_8.rs"));
}

pub mod transaction {
    include!(concat!(env!("OUT_DIR"), "/transaction.rs"));
}

pub mod version {
    include!(concat!(env!("OUT_DIR"), "/versionpb.rs"));
}
