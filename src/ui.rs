pub mod functions;
pub mod handler;
pub mod model;

#[allow(clippy::all)]
pub mod generated {
    slint::include_modules!();
}

pub use generated::*;
