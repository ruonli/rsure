// Errors.

use failure;
use std::process::ExitStatus;
use std::result;

pub type Result<T> = result::Result<T, Error>;
pub type Error = failure::Error;

#[derive(Fail, Debug)]
pub enum WeaveError {
    #[fail(display = "Error running BitKeeper: {:?}: {:?}", _0, _1)]
    BkError(ExitStatus, String),
}
