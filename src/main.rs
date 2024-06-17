mod fmt;
mod mode;
mod process;

use mode::{mode_manager, Mode};
use process::Process;
use serde::Deserialize;
use std::{
    fs::{create_dir_all, read_to_string},
    path::Path,
};
use tokio::task::JoinSet;

#[derive(Deserialize, Debug)]
pub struct Config {
    pub _addr: String,
    pub proc: Vec<Process>,
}

fn main() {
    let path = std::env::args()
        .nth(1)
        .expect("Usage: supervisor config.toml");
    let text = match read_to_string(path) {
        Ok(text) => text,
        Err(e) => panic!("Failed to read config file: {e}"),
    };
    let config = match toml::from_str(&text) {
        Ok(config) => config,
        Err(e) => panic!("Config file is inllegal: {e}"),
    };

    let log = Path::new("log");
    create_dir_all(log).unwrap();

    tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(run(log, config));
}

async fn run(log: impl AsRef<Path>, Config { _addr, proc }: Config) {
    let mut set = JoinSet::new();
    for proc in proc {
        let (_subscriber, listener) = mode_manager(Mode::Once);
        let log = log.as_ref().to_path_buf();
        set.spawn(async move { proc.run(log, listener).await });
    }
    while set.join_next().await.is_some() {}
}
