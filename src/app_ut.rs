use std::{fs, path::Path};

use crate::{
    app::{JobEvent, JobState, LOG_READ_MAX, LOG_TICK_MAX},
    model::{BuildTarget, ResultMirrorPlan, XrunConfig},
    runner::BuildJob,
};

#[test]
fn job_state_rebuilds_rendered_lines_only_when_log_changes() {
    let tmp = std::env::temp_dir().join(format!("xrun-app-ut-{}-{}", std::process::id(), uniq()));
    let logs = tmp.join("logs");
    let job = BuildJob::build(
        &BuildTarget::local(),
        "devel",
        &tmp,
        &logs,
        "make",
        &ResultMirrorPlan::disabled(tmp.join("mirror"), "devel"),
    );
    let log = job.log_path().to_path_buf();
    let mut st = JobState::from_job(&job);

    fs::create_dir_all(&logs).expect("log dir should exist");
    fs::write(&log, "first\n").expect("log should be written");
    assert!(st.refresh_log());
    assert_eq!(st.log_lines().len(), 2);
    assert_eq!(st.log_lines()[0].spans[0].content, "first");

    let before = st.log_lines().to_vec();
    assert!(!st.refresh_log());
    assert_eq!(st.log_lines(), before.as_slice());

    fs::write(&log, "first\nsecond\n").expect("log should be appended");
    assert!(st.refresh_log());
    assert_eq!(st.log_lines().len(), 3);
    assert_eq!(st.log_lines()[1].spans[0].content, "second");

    fs::remove_dir_all(&tmp).ok();
}

#[test]
fn job_state_keeps_running_stage_until_log_drains() {
    let tmp = std::env::temp_dir().join(format!("xrun-app-ut-{}-{}", std::process::id(), uniq()));
    let logs = tmp.join("logs");
    let job = BuildJob::build(
        &BuildTarget::local(),
        "devel",
        &tmp,
        &logs,
        "make",
        &ResultMirrorPlan::disabled(tmp.join("mirror"), "devel"),
    );
    let log = job.log_path().to_path_buf();
    let mut st = JobState::from_job(&job);
    let text = "x".repeat(LOG_TICK_MAX + 128);

    fs::create_dir_all(&logs).expect("log dir should exist");
    fs::write(&log, &text).expect("log should be written");

    st.apply(JobEvent::building(0));
    assert!(st.refresh_log());
    st.apply(JobEvent::finished(0, 0));

    assert!(!st.is_finished());
    assert_eq!(st.summary(), "building");

    assert!(st.refresh_log());

    assert!(st.is_finished());
    assert!(st.is_success());
    assert_eq!(st.summary(), "finished");

    fs::remove_dir_all(&tmp).ok();
}

#[test]
fn job_state_drains_many_chunks_in_one_tick() {
    let tmp = std::env::temp_dir().join(format!("xrun-app-ut-{}-{}", std::process::id(), uniq()));
    let logs = tmp.join("logs");
    let job = BuildJob::build(
        &BuildTarget::local(),
        "devel",
        &tmp,
        &logs,
        "make",
        &ResultMirrorPlan::disabled(tmp.join("mirror"), "devel"),
    );
    let log = job.log_path().to_path_buf();
    let mut st = JobState::from_job(&job);
    let text = "x".repeat(LOG_READ_MAX * 3 + 7);

    fs::create_dir_all(&logs).expect("log dir should exist");
    fs::write(&log, &text).expect("log should be written");

    assert!(st.refresh_log());
    assert_eq!(st.log_lines()[0].spans[0].content.len(), text.len());

    fs::remove_dir_all(&tmp).ok();
}

#[test]
fn job_state_updates_rendered_lines_for_error_events() {
    let tmp = std::env::temp_dir().join(format!("xrun-app-ut-{}-{}", std::process::id(), uniq()));
    let plan = crate::runner::BuildPlan::new(
        &XrunConfig::parse("local\n").expect("config should parse"),
        "devel",
        Path::new(&tmp),
        &tmp.join("logs"),
        "make",
        ResultMirrorPlan::disabled(tmp.join("mirror"), "devel"),
    );
    let mut st = JobState::from_job(&plan.jobs()[0]);

    st.apply(JobEvent::failed(0, "boom".to_string()));

    assert!(
        st.log_lines()
            .iter()
            .flat_map(|line| line.spans.iter())
            .any(|span| span.content.contains("boom"))
    );

    fs::remove_dir_all(&tmp).ok();
}

fn uniq() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("time should advance")
        .as_nanos()
        .to_string()
}
