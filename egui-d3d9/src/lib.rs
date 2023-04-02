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
mod inputman;
mod mesh;
mod state;
mod texman;

pub use app::*;
