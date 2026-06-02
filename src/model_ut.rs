use std::path::Path;

use crate::model::{ResultMirrorPlan, TargetMode, MxrunConfig};

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
