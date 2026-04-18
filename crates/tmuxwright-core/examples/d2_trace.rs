// Manual validation for d2-trace.
// Run: cargo run -p tmuxwright-core --example d2_trace

use std::time::Duration;

use tmuxwright_core::{
    Action, EngineError, Preservation, Recorder, RegionRecord, Selector, Snapshot, Via,
};

fn main() {
    let out_dir = std::path::PathBuf::from("tmp/d2-trace/artifacts");
    let _ = std::fs::remove_dir_all(&out_dir);

    let mut r = Recorder::new().with_artifact_dir(&out_dir);

    let s0 = Snapshot::from_plain(12, 1, "prompt>     ");
    let s1 = Snapshot::from_plain(12, 1, "prompt> hi  ");
    let s2 = Snapshot::from_plain(12, 1, "hello!      ");

    r.record_action(&Action::Type("hi".into()), Some(&s0), &s1)
        .unwrap();
    r.record_wait(
        "stable(quiet=100ms)",
        "satisfied",
        Duration::from_millis(140),
        &s1.hash.hex(),
    );
    r.record_action(&Action::Press(tmuxwright_core::Key::Enter), Some(&s1), &s2)
        .unwrap();
    r.record_resolve(
        &Selector::Text {
            needle: "hello".into(),
            case_insensitive: false,
            nth: 0,
        },
        Via::Terminal,
        RegionRecord {
            x: 0,
            y: 0,
            width: 5,
            height: 1,
        },
    );
    r.record_assert("screen contains 'hello'", true, &s2);

    let err = EngineError::WaitTimeout {
        condition: tmuxwright_core::WaitCondition::Text {
            needle: "bye".into(),
            case_insensitive: false,
        },
        waited: Duration::from_secs(5),
        preservation: Some(Preservation::new("tmuxwright-xyz", "run-1")),
    };
    r.record_error(&err);

    let trace_path = r.persist_trace().unwrap().unwrap();
    println!("trace at {}", trace_path.display());
    let contents = std::fs::read_to_string(&trace_path).unwrap();
    for (i, line) in contents.lines().enumerate() {
        println!("[{i}] {line}");
    }

    println!("\nartifacts:");
    for e in std::fs::read_dir(&out_dir).unwrap() {
        let p = e.unwrap().path();
        println!("  {}", p.file_name().unwrap().to_string_lossy());
    }
}
