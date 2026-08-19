use std::path::Path;

use crate::model::{MxrunConfig, ResultMirrorPlan, TargetMode};

#[test]
fn parse_accepts_local_pseudo_host() {
    let cfg = MxrunConfig::parse("local\n").expect("local target should parse");

    assert_eq!(cfg.targets().len(), 1);
    assert!(cfg.targets()[0].is_local());
    assert_eq!(cfg.targets()[0].mode(), &TargetMode::Local);
    assert!(!cfg.targets()[0].os().is_empty());
    assert!(!cfg.targets()[0].arch().is_empty());
    assert_eq!(cfg.targets()[0].destination(), "local");
}

#[test]
fn parse_accepts_remote_targets() {
    let cfg = MxrunConfig::parse("FreeBSD amd64 192.168.122.122:work/sysinspect-mxrun\n")
        .expect("remote target should parse");

    assert_eq!(cfg.targets().len(), 1);
    assert_eq!(cfg.targets()[0].mode(), &TargetMode::Remote);
    assert_eq!(cfg.targets()[0].os(), "FreeBSD");
    assert_eq!(cfg.targets()[0].arch(), "amd64");
    assert_eq!(
        cfg.targets()[0].destination(),
        "192.168.122.122:work/sysinspect-mxrun"
    );
}

#[test]
fn parse_keeps_comments_and_blank_lines_ignored() {
    let cfg =
        MxrunConfig::parse("\n# comment\nlocal\n\nGNU/Linux x86_64 bo@jackass:work/sysinspect\n")
            .expect("mixed config should parse");

    assert_eq!(cfg.targets().len(), 2);
}

#[test]
fn parse_accepts_yaml_targets_and_project_ignores() {
    let cfg = MxrunConfig::parse(
        "targets:\n  - local\n  - FreeBSD amd64 builder@freebsd-vm:work/demo\n\nproject:\n  ignore:\n    - /generated/\n    - '*.cache'\n  in-a-future:\n    other: option\n",
    )
    .expect("YAML config should parse");

    assert_eq!(cfg.targets().len(), 2);
    assert_eq!(cfg.ignores(), ["/generated/", "*.cache"]);
}

#[test]
fn parse_yaml_rejects_non_string_ignore_patterns() {
    let err = MxrunConfig::parse("targets:\n  - local\nproject:\n  ignore:\n    - 1\n")
        .expect_err("ignore patterns must be strings");

    assert!(err.contains("invalid YAML mxrun config"));
}

#[test]
fn parse_accepts_project_build_output() {
    let cfg = MxrunConfig::parse(
        "targets:\n  - local\nproject:\n  build:\n    src:\n      - target/debug/example\n      - build/output\n    files:\n      - example\n    dst: target/platforms\n",
    )
    .expect("build output config should parse");
    let output = cfg
        .build_output()
        .expect("build output should be configured");

    assert_eq!(
        output.sources(),
        [Path::new("target/debug/example"), Path::new("build/output")]
    );
    assert_eq!(output.files(), [Path::new("example")]);
    assert!(output.selects_files());
    assert_eq!(output.destination(), Path::new("target/platforms"));
}

#[test]
fn parse_rejects_unsafe_project_build_source() {
    let err = MxrunConfig::parse(
        "targets:\n  - local\nproject:\n  build:\n    src:\n      - ../outside\n    dst: target/platforms\n",
    )
    .expect_err("source paths must remain in the workspace");

    assert!(err.contains("expected a non-empty relative path without '..'"));
}

#[test]
fn parse_rejects_project_build_destination_inside_source() {
    let err = MxrunConfig::parse(
        "targets:\n  - local\nproject:\n  build:\n    src:\n      - target\n    dst: target/platforms\n",
    )
    .expect_err("destination must not recursively copy into a source");

    assert!(err.contains("dst must not be inside a configured src path"));
}

#[test]
fn parse_rejects_unsafe_project_build_file_selector() {
    let err = MxrunConfig::parse(
        "targets:\n  - local\nproject:\n  build:\n    src:\n      - target/debug\n    files:\n      - ../outside\n    dst: target/platforms\n",
    )
    .expect_err("file selectors must remain in their source directory");

    assert!(err.contains("invalid project build file"));
}

#[test]
fn parse_treats_an_empty_file_selector_as_defined() {
    let cfg = MxrunConfig::parse(
        "targets:\n  - local\nproject:\n  build:\n    src:\n      - target/debug\n    files: []\n    dst: target/platforms\n",
    )
    .expect("build output config should parse");

    assert!(
        cfg.build_output()
            .expect("build output should be configured")
            .selects_files()
    );
}

#[test]
fn parse_rejects_bad_field_count() {
    let err = MxrunConfig::parse("FreeBSD amd64\n").expect_err("bad field count must fail");

    assert!(err.contains("expected 3 fields"));
}

#[test]
fn parse_rejects_missing_destination_separator() {
    let err = MxrunConfig::parse("FreeBSD amd64 192.168.122.122\n")
        .expect_err("missing host:/destination separator must fail");

    assert!(err.contains("missing host:/destination"));
}

#[test]
fn parse_rejects_empty_config() {
    let err = MxrunConfig::parse("\n# comment\n\n").expect_err("empty config must fail");

    assert_eq!(err, "mxrun config has no targets");
}

#[test]
fn result_mirror_plan_uses_standard_manifest_path() {
    let plan = ResultMirrorPlan::new(true, "/tmp/mxrun".into(), "dev");

    assert!(plan.is_enabled());
    assert_eq!(plan.manifest(), Path::new("build/.mxrun/dev.paths"));
    assert_eq!(plan.root(), Path::new("/tmp/mxrun"));
}
