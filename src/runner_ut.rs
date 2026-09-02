use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use crate::{
    model::{BuildTarget, MxrunConfig, ResultMirrorPlan},
    runner::{BuildCommand, BuildJob, BuildPlan, MakeVariable},
};

#[test]
fn local_job_writes_full_log_file_from_pty() {
    let log_path = TempDir::new("mxrun-runner-ut").path().join("local.log");
    let job = BuildJob::new(
        BuildTarget::local(),
        BuildCommand::new(
            "sh",
            vec![
                "-lc".to_string(),
                "printf '\\033[1;31mRED\\033[0m\\n'".to_string(),
            ],
            None,
        ),
        log_path.clone(),
        PathBuf::from("/tmp/sysinspect"),
        ResultMirrorPlan::disabled(PathBuf::from("/tmp/mxrun"), "dev"),
        crate::runner::RunMode::Run,
    );
    let result = job.run().expect("local PTY job should run");

    assert!(result.is_success());
    assert_eq!(result.status(), 0);
    assert_eq!(result.log_path(), Path::new(&log_path));
    assert!(job.target().is_local());
    assert!(
        fs::read_to_string(&log_path)
            .expect("log file should exist")
            .contains("\u{1b}[1;31mRED")
    );
}

#[test]
fn remote_job_uses_ssh_tty_and_remote_make_command() {
    let job = BuildJob::build(
        &BuildTarget::remote("FreeBSD", "amd64", "192.168.122.122:work/sysinspect-mxrun"),
        "dev",
        Path::new("/tmp/sysinspect"),
        Path::new("/tmp/logs"),
        "make",
        &ResultMirrorPlan::disabled(PathBuf::from("/tmp/mxrun"), "dev"),
    );
    let command = job.command().args();

    assert_eq!(command[0], "-o");
    assert_eq!(command[1], "StrictHostKeyChecking=accept-new");
    assert_eq!(command[2], "-o");
    assert_eq!(command[3], "UpdateHostKeys=yes");
    assert_eq!(command[4], "-tt");
    assert_eq!(command[5], "192.168.122.122");
    assert_eq!(command[6], "cd 'work/sysinspect-mxrun' && gmake dev");
}

#[test]
fn build_plan_creates_one_job_per_target_with_stable_log_paths() {
    let temp = TempDir::new("mxrun-plan-ut");
    let plan = BuildPlan::new(
        &MxrunConfig::parse("local\nFreeBSD amd64 192.168.122.122:work/sysinspect-mxrun\n")
            .expect("config should parse"),
        "modules-dev",
        Path::new("/tmp/sysinspect"),
        temp.path(),
        "make",
        ResultMirrorPlan::disabled(PathBuf::from("/tmp/mxrun"), "modules-dev"),
        &[],
    );

    assert_eq!(plan.jobs().len(), 2);
    assert_eq!(plan.jobs()[0].log_path(), temp.path().join("local.log"));
    assert_eq!(
        plan.jobs()[1].log_path(),
        temp.path()
            .join("192.168.122.122_work_sysinspect-mxrun.log")
    );
}

#[test]
fn make_variable_parses_plain_and_special_values() {
    assert!(MakeVariable::parse("HOST=deployer@example.test".to_string()).is_ok());
    assert!(MakeVariable::parse("MESSAGE=it's got 'spaces'".to_string()).is_ok());
}

#[test]
fn make_variable_rejects_invalid_assignments() {
    assert_eq!(
        MakeVariable::parse("HOST".to_string()).unwrap_err(),
        "mxrun: --make-var must use NAME=VALUE"
    );
    assert_eq!(
        MakeVariable::parse("=deployer".to_string()).unwrap_err(),
        "mxrun: --make-var NAME must be a Make identifier"
    );
    assert_eq!(
        MakeVariable::parse("1HOST=x".to_string()).unwrap_err(),
        "mxrun: --make-var NAME must be a Make identifier"
    );
    assert_eq!(
        MakeVariable::parse("HOST-NAME=x".to_string()).unwrap_err(),
        "mxrun: --make-var NAME must be a Make identifier"
    );
}

#[test]
fn make_variable_accepts_permissive_names_and_empty_values() {
    assert!(MakeVariable::parse("HOST=".to_string()).is_ok());
    assert!(MakeVariable::parse("_HOST=x".to_string()).is_ok());
    assert!(MakeVariable::parse("HOST1=x".to_string()).is_ok());
}

#[test]
fn local_command_appends_quoted_make_vars_after_entry() {
    let vars = vec![
        MakeVariable::parse("HOST=deployer@example.test".to_string())
            .expect("assignment should parse"),
        MakeVariable::parse("BUILD_KIND=debug".to_string()).expect("assignment should parse"),
        MakeVariable::parse("MESSAGE=it's got 'spaces'".to_string())
            .expect("assignment should parse"),
    ];

    let command = BuildCommand::for_target(
        &BuildTarget::local(),
        "_push-dev",
        Path::new("/tmp/kenpit"),
        "make",
        &vars,
    );

    assert_eq!(command.program(), "sh");
    assert_eq!(command.cwd(), Some(Path::new("/tmp/kenpit")));
    assert_eq!(
        command.args(),
        [
            "-lc",
            "MXRUN_CONFIG= MXRUN_LOCAL_MAKE= make _push-dev 'HOST=deployer@example.test' 'BUILD_KIND=debug' 'MESSAGE=it'\\''s got '\\''spaces'\\'''"
        ]
    );
}

#[test]
fn remote_command_appends_quoted_make_vars_after_entry() {
    let vars = vec![
        MakeVariable::parse("HOST=deployer@example.test".to_string())
            .expect("assignment should parse"),
        MakeVariable::parse("BUILD_KIND=debug".to_string()).expect("assignment should parse"),
    ];

    let command = BuildCommand::for_target(
        &BuildTarget::remote("FreeBSD", "amd64", "192.168.122.122:work/sysinspect-mxrun"),
        "_push-dev",
        Path::new("/tmp/sysinspect"),
        "make",
        &vars,
    );

    assert_eq!(command.program(), "ssh");
    assert_eq!(command.args()[0], "-o");
    assert_eq!(command.args()[1], "StrictHostKeyChecking=accept-new");
    assert_eq!(command.args()[2], "-o");
    assert_eq!(command.args()[3], "UpdateHostKeys=yes");
    assert_eq!(command.args()[4], "-tt");
    assert_eq!(command.args()[5], "192.168.122.122");
    assert_eq!(
        command.args()[6],
        "cd 'work/sysinspect-mxrun' && gmake _push-dev 'HOST=deployer@example.test' 'BUILD_KIND=debug'"
    );
}

#[test]
fn remote_command_without_vars_keeps_existing_gmake_dev_form() {
    let command = BuildCommand::for_target(
        &BuildTarget::remote("FreeBSD", "amd64", "192.168.122.122:work/sysinspect-mxrun"),
        "dev",
        Path::new("/tmp/sysinspect"),
        "make",
        &[],
    );

    assert_eq!(command.args()[6], "cd 'work/sysinspect-mxrun' && gmake dev");
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
