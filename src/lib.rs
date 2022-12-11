mod local;
mod remote;

pub use local::Client;
pub use remote::RemoteClient;

use reqwest::Url;
use std::path::PathBuf;

#[derive(Copy, Clone)]
enum Format {
    Html,
    Text,
    Mime,
    Metadata,
}

#[derive(Debug)]
pub enum Input {
    FilePath(PathBuf),
    Url(Url),
}
