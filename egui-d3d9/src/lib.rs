#![feature(once_cell)]

macro_rules! expect {
    ($val:expr, $msg:expr) => {
        if cfg!(feature = "silent") {
            $val.unwrap()
        } else {
            $val.expect($msg)
        }
    };
}

mod app;
mod backup;
mod inputman;
mod mesh;
mod texman;

pub use app::*;
