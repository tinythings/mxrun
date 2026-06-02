mod ansi;
#[cfg(test)]
mod ansi_ut;
mod app;
#[cfg(test)]
mod app_ut;
mod clidef;
mod model;
#[cfg(test)]
mod model_ut;
mod runner;
#[cfg(test)]
mod runner_ut;
mod ui;
#[cfg(test)]
mod ui_ut;

use std::path::{Path, PathBuf};
use std::process::{Command as ProcessCommand, Stdio};
use std::{env, fs, process};

use clap::ArgMatches;

use app::MxrunApp;
use model::{ResultMirrorPlan, MxrunConfig};
use runner::BuildPlan;

pub(crate) const VERSION: &str = env!("CARGO_PKG_VERSION");

struct App;

impl App {
    fn run() -> ! {
        process::exit(Self::command().run());
    }

    fn command() -> Command {
        Command::from_matches(clidef::cli().get_matches())
    }
}

enum Command {
    AddHost(AddHostOptions),
    Init,
    Run(RunOptions),
}

struct AddHostOptions {
    host: String,
}

struct RunOptions {
    entry: String,
    mirror_results: bool,
    mirror_root: PathBuf,
    wrap_lines: bool,
}

impl Command {
    fn from_matches(am: ArgMatches) -> Self {
        if let Some(host) = clidef::add_host(&am) {
            return Self::AddHost(AddHostOptions { host });
        }
        match am.subcommand_name() {
            Some("init") => Self::Init,
            Some("run") => Self::Run(RunOptions::from_matches(&am)),
            _ => Self::usage(),
        }
    }

    fn run(&self) -> i32 {
        match self {
            Self::AddHost(options) => options.run(),
            Self::Init => self.init(),
            Self::Run(options) => self.run_entry(options),
        }
    }

    fn init(&self) -> i32 {
        MxrunApp::new(
            BuildPlan::init(&ConfigFile::load(), &RepoRoot::path()),
            false,
        )
        .run()
        .unwrap_or_else(|err| Fatal::raise(&err))
    }

    fn run_entry(&self, options: &RunOptions) -> i32 {
        options.announce_mirroring_contract();
        MxrunApp::new(
            BuildPlan::new(
                &ConfigFile::load(),
                options.entry(),
                &RepoRoot::path(),
                &LogRoot::path(options.entry()),
                &LocalMake::name(),
                options.mirror_plan(),
            ),
            options.wrap_lines(),
        )
        .run()
        .unwrap_or_else(|err| Fatal::raise(&err))
    }

    fn usage() -> ! {
        Fatal::raise("Usage: mxrun [--add-host|-a <host>] init | run <entry>")
    }
}

impl AddHostOptions {
    fn run(&self) -> i32 {
        self.add_host()
            .map(|line| {
                eprintln!("mxrun: added host target: {line}");
                0
            })
            .unwrap_or_else(|err| Fatal::raise(&err))
    }

    fn add_host(&self) -> Result<String, String> {
        let user = LocalUser::name()?;
        let remote = format!("{user}@{}", self.host());
        SshCopyId::run(&remote)?;
        let os = RemoteProbe::capture(&remote, &["uname", "-o"])?;
        let arch = RemoteProbe::capture(&remote, &["uname", "-m"])?;
        let destination = RepoRoot::path();
        let line = ConfigLine::remote(&os, &arch, &remote, &destination);

        ConfigFile::append_target_line_at(&PathBuf::from(ConfigFile::path()), &line)?;
        Ok(line)
    }

    fn host(&self) -> &str {
        &self.host
    }
}

impl RunOptions {
    fn from_matches(am: &ArgMatches) -> Self {
        let mirror_results = clidef::mirror_results(am);
        let mirror_root = clidef::mirror_root(am);
        if mirror_root.is_some() && !mirror_results {
            Fatal::raise("mxrun: --mirror-root requires --mirror-results");
        }
        Self {
            entry: clidef::entry(am),
            mirror_results,
            mirror_root: mirror_root.unwrap_or_else(Self::default_mirror_root),
            wrap_lines: clidef::wrap_lines(am),
        }
    }

    fn default_mirror_root() -> PathBuf {
        RepoRoot::path().join("target").join("mxrun")
    }

    fn entry(&self) -> &str {
        &self.entry
    }

    fn mirror_results(&self) -> bool {
        self.mirror_results
    }

    fn mirror_root(&self) -> &PathBuf {
        &self.mirror_root
    }

    fn wrap_lines(&self) -> bool {
        self.wrap_lines
    }

    fn announce_mirroring_contract(&self) {
        if self.mirror_results() {
            eprintln!(
                "mxrun: result mirroring requested; local mirror root is {}",
                self.mirror_root().display()
            );
        }
    }

    fn mirror_plan(&self) -> ResultMirrorPlan {
        ResultMirrorPlan::new(
            self.mirror_results(),
            self.mirror_root().clone(),
            self.entry(),
        )
    }
}

struct ConfigFile;

impl ConfigFile {
    fn path() -> String {
        env::args()
            .skip(1)
            .collect::<Vec<_>>()
            .windows(2)
            .find(|args| args[0] == "-c" || args[0] == "--config")
            .map(|args| args[1].clone())
            .or_else(|| env::var("MXRUN_CONFIG").ok())
            .unwrap_or_else(|| Fatal::raise("MXRUN_CONFIG is not set"))
    }

    fn read() -> String {
        Self::read_or_create(&PathBuf::from(Self::path()))
            .unwrap_or_else(|err| Fatal::raise(&format!("mxrun: failed to read config: {err}")))
    }

    fn load() -> MxrunConfig {
        MxrunConfig::parse(&Self::read()).unwrap_or_else(|err| Fatal::raise(&format!("mxrun: {err}")))
    }

    fn read_or_create(path: &PathBuf) -> Result<String, String> {
        fs::read_to_string(path)
            .or_else(|err| {
                if err.kind() == std::io::ErrorKind::NotFound {
                    Self::create_default(path).map(|_| Self::default_contents().to_string())
                } else {
                    Err(err)
                }
            })
            .map_err(|err| err.to_string())
    }

    fn create_default(path: &PathBuf) -> Result<(), std::io::Error> {
        path.parent()
            .into_iter()
            .try_for_each(fs::create_dir_all)
            .and_then(|_| fs::write(path, Self::default_contents()))
    }

    fn append_target_line_at(path: &PathBuf, line: &str) -> Result<(), String> {
        let mut text = Self::read_or_create(path)?;

        if text.lines().any(|existing| existing.trim() == line) {
            return Ok(());
        }

        if !text.ends_with('\n') {
            text.push('\n');
        }
        text.push_str(line);
        text.push('\n');

        fs::write(path, text).map_err(|err| err.to_string())
    }

    fn default_contents() -> &'static str {
        "local\n"
    }
}

struct RepoRoot;

impl RepoRoot {
    fn path() -> std::path::PathBuf {
        std::env::current_dir().unwrap_or_else(|err| {
            Fatal::raise(&format!("mxrun: failed to detect current directory: {err}"))
        })
    }
}

struct LogRoot;

impl LogRoot {
    fn path(entry: &str) -> std::path::PathBuf {
        RepoRoot::path().join(".mxrun").join("logs").join(entry)
    }
}

struct LocalMake;

impl LocalMake {
    fn name() -> String {
        env::var("MXRUN_LOCAL_MAKE").unwrap_or_else(|_| "make".to_string())
    }
}

struct LocalUser;

impl LocalUser {
    fn name() -> Result<String, String> {
        env::var("USER")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .or_else(|| Self::capture_id_user().ok())
            .ok_or_else(|| "mxrun: failed to detect local user name".to_string())
    }

    fn capture_id_user() -> Result<String, String> {
        ProcessCommand::new("id")
            .arg("-un")
            .output()
            .map_err(|err| format!("mxrun: failed to run 'id -un': {err}"))
            .and_then(|output| {
                output
                    .status
                    .success()
                    .then_some(String::from_utf8_lossy(&output.stdout).trim().to_string())
                    .filter(|value| !value.is_empty())
                    .ok_or_else(|| "mxrun: failed to detect local user name".to_string())
            })
    }
}

struct SshCopyId;

impl SshCopyId {
    fn run(remote: &str) -> Result<(), String> {
        ProcessCommand::new("ssh-copy-id")
            .arg(remote)
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
            .map_err(|err| format!("mxrun: failed to run ssh-copy-id: {err}"))
            .and_then(|status| {
                status.success().then_some(()).ok_or_else(|| {
                    format!(
                        "mxrun: ssh-copy-id failed with status {}",
                        status.code().unwrap_or(1)
                    )
                })
            })
    }
}

struct RemoteProbe;

impl RemoteProbe {
    fn capture(remote: &str, command: &[&str]) -> Result<String, String> {
        ProcessCommand::new("ssh")
            .arg(remote)
            .args(command)
            .output()
            .map_err(|err| format!("mxrun: failed to query {remote}: {err}"))
            .and_then(|output| {
                output
                    .status
                    .success()
                    .then_some(String::from_utf8_lossy(&output.stdout).trim().to_string())
                    .filter(|value| !value.is_empty())
                    .ok_or_else(|| {
                        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                        format!("mxrun: failed to query {remote}: {stderr}")
                    })
            })
    }
}

struct ConfigLine;

impl ConfigLine {
    fn remote(os: &str, arch: &str, remote: &str, destination: &Path) -> String {
        format!("{os} {arch} {remote}:{}", destination.display())
    }
}

struct Fatal;

impl Fatal {
    fn raise(msg: &str) -> ! {
        eprintln!("{msg}");
        process::exit(2);
    }
}

fn main() {
    App::run();
}

#[cfg(test)]
mod main_ut {
    use super::{ConfigFile, ConfigLine};
    use std::{
        fs,
        path::{Path, PathBuf},
        time::{SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn read_or_create_bootstraps_missing_config_with_local_target() {
        let temp = TempDir::new("mxrun-main-ut");
        let path = temp.path().join("nested").join("mxrun.conf");

        let text = ConfigFile::read_or_create(&path).expect("missing config should be created");

        assert_eq!(text, "local\n");
        assert_eq!(
            fs::read_to_string(&path).expect("created config should exist"),
            "local\n"
        );
    }

    #[test]
    fn read_or_create_keeps_existing_config_contents() {
        let temp = TempDir::new("mxrun-main-ut");
        let path = temp.path().join("mxrun.conf");
        fs::write(&path, "local\nFreeBSD amd64 host:work/tree\n")
            .expect("fixture config should be written");

        let text = ConfigFile::read_or_create(&path).expect("existing config should be read");

        assert_eq!(text, "local\nFreeBSD amd64 host:work/tree\n");
    }

    #[test]
    fn append_target_line_adds_new_remote_entry_once() {
        let temp = TempDir::new("mxrun-main-ut");
        let path = temp.path().join("mxrun.conf");
        fs::write(&path, "local\n").expect("fixture config should be written");

        let line = "GNU/Linux x86_64 bo@example:/home/bo/work/mxrun";
        ConfigFile::append_target_line_at(&path, line).expect("remote target should be appended");
        ConfigFile::append_target_line_at(&path, line).expect("duplicate append should be ignored");

        assert_eq!(
            fs::read_to_string(&path).expect("config should be readable"),
            "local\nGNU/Linux x86_64 bo@example:/home/bo/work/mxrun\n"
        );
    }

    #[test]
    fn config_line_uses_current_project_path_as_destination() {
        let line = ConfigLine::remote(
            "GNU/Linux",
            "x86_64",
            "bo@example",
            Path::new("/home/bo/work/mxrun"),
        );

        assert_eq!(line, "GNU/Linux x86_64 bo@example:/home/bo/work/mxrun");
    }

    struct TempDir {
        path: PathBuf,
    }

    impl TempDir {
        fn new(prefix: &str) -> Self {
            Self {
                path: std::env::temp_dir().join(format!(
                    "{prefix}-{}-{}",
                    std::process::id(),
                    SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .expect("clock should move forward")
                        .as_nanos()
                )),
            }
            .create()
        }

        fn create(self) -> Self {
            fs::create_dir_all(&self.path).expect("temp dir should be created");
            self
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}
