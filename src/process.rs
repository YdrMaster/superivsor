use crate::{
    fmt::format_time,
    mode::{Listener, Mode},
};
use serde::{Deserialize, Serialize};
use std::{
    ffi::OsString,
    fmt,
    path::{Path, PathBuf},
    process::{ExitStatus, Stdio},
};
use time::OffsetDateTime;
use tokio::{
    fs::File,
    io::{AsyncBufReadExt, AsyncRead, AsyncWriteExt, BufReader},
    process::Command,
};
use toml::value::Array;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Process {
    executable: String,
    args: Array,
}

impl Process {
    #[inline]
    pub fn executable(&self) -> &Path {
        Path::new(&self.executable)
    }

    #[inline]
    pub fn args<'s>(&'s self) -> Vec<OsString> {
        use toml::Value as Toml;

        fn fmt_primitive(value: &toml::value::Value) -> OsString {
            match value {
                Toml::String(s) => OsString::from(s),
                Toml::Integer(i) => OsString::from(i.to_string()),
                Toml::Float(f) => OsString::from(f.to_string()),
                Toml::Boolean(b) => OsString::from(b.to_string()),
                _ => unimplemented!(),
            }
        }

        let mut ans = Vec::new();
        for arg in &self.args {
            match arg {
                Toml::String(_) | Toml::Integer(_) | Toml::Float(_) | Toml::Boolean(_) => {
                    ans.push(fmt_primitive(arg));
                }
                Toml::Table(table) => {
                    for (k, v) in table {
                        match v {
                            Toml::String(_) | Toml::Integer(_) | Toml::Float(_) => {
                                ans.push(OsString::from(format!("--{k}")));
                                ans.push(fmt_primitive(arg));
                            }
                            Toml::Boolean(true) => ans.push(OsString::from(format!("--{k}"))),
                            Toml::Boolean(false) => {}
                            Toml::Datetime(_) | Toml::Array(_) | Toml::Table(_) => unimplemented!(),
                        }
                    }
                }
                Toml::Datetime(_) | Toml::Array(_) => unimplemented!(),
            }
        }
        ans
    }

    pub async fn run(&self, log: PathBuf, listener: Listener) {
        let args = self.args();

        loop {
            let _ = listener.wait_for(|m| !matches!(m, Mode::Stop)).await;

            let mut cmd = Command::new(&self.executable);
            cmd.args(&args)
                .kill_on_drop(true)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());

            let time = format_time(OffsetDateTime::now_utc());
            let mut child = match cmd.spawn() {
                Ok(child) => child,
                Err(e) => {
                    println!("Command failed to start: {e:?}, never restart.");
                    break;
                }
            };

            async fn save(path: impl AsRef<Path>, channel: impl AsyncRead + Unpin) {
                let mut file = File::create(path).await.unwrap();
                let mut lines = BufReader::new(channel).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    file.write_all(line.as_bytes()).await.unwrap();
                }
            }
            {
                let path = log.join(format!("{}-stdout-{time}.log", self.executable));
                let channel = child.stdout.take().unwrap();
                tokio::spawn(async move { save(path, channel).await });
            }
            {
                let path = log.join(format!("{}-stderr-{time}.log", self.executable));
                let channel = child.stderr.take().unwrap();
                tokio::spawn(async move { save(path, channel).await });
            }

            let ending = tokio::select! {
                ending = child.wait() => {
                    listener.compair_exchange(Mode::Once, Mode::Stop);
                    Ending::Finnish(ending.unwrap())
                }
                _ = listener.wait_for(|m| matches!(m, Mode::Stop)) => {
                    child.kill().await.unwrap();
                    Ending::Killed
                }
            };
            {
                let path = log.join(format!("{}-status-{time}.log", self.executable));
                let proc = self.clone();
                let mode = listener.get();
                tokio::spawn(async move {
                    let content = format!("{}\n", Info { proc, ending, mode });
                    tokio::fs::write(path, content.as_bytes()).await.unwrap();
                });
            }
        }
    }
}

struct Info {
    pub proc: Process,
    pub ending: Ending,
    pub mode: Mode,
}

enum Ending {
    Finnish(ExitStatus),
    Killed,
}

impl fmt::Display for Info {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let proc = toml::to_string_pretty(&self.proc).unwrap();

        writeln!(f, "{proc}")?;
        writeln!(f, "[status]")?;
        match self.ending {
            Ending::Finnish(s) => {
                writeln!(
                    f,
                    "status = {}",
                    if s.success() { "success" } else { "failure" }
                )?;
                if let Some(code) = s.code() {
                    writeln!(f, "exit-code = {code}")?;
                }
            }
            Ending::Killed => {
                writeln!(f, "status = killed")?;
            }
        }
        writeln!(f, "mode = {:?}", self.mode)?;
        writeln!(f, "---")?;
        writeln!(f)
    }
}
