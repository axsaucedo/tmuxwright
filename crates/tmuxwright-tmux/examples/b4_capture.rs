// Manual validation for b4-capture.
//
// Run: cargo run -p tmuxwright-tmux --example b4_capture

use std::thread::sleep;
use std::time::Duration;

use tmuxwright_tmux::{
    capture_visible_plain, capture_with_scrollback_ansi, detect, pane_geometry, send_keys,
    type_text, Key, Session, SessionOptions,
};

fn main() {
    let tmux = detect().expect("tmux");
    let opts = SessionOptions {
        width: 80,
        height: 24,
        command: vec!["bash".into(), "--noprofile".into(), "--norc".into()],
    };
    let s = Session::create(tmux, &opts).expect("session");
    sleep(Duration::from_millis(200));

    // Generate enough output to force scrollback.
    type_text(
        &s,
        "PS1='$ '\nfor i in $(seq 1 60); do echo line_$i; done\n",
    )
    .unwrap();
    sleep(Duration::from_millis(400));

    println!("== visible plain ==");
    println!("{}", capture_visible_plain(&s).unwrap());

    println!("\n== visible + scrollback with ANSI (len, first 120 bytes) ==");
    let full = capture_with_scrollback_ansi(&s).unwrap();
    println!("length: {}", full.len());
    println!("head  : {:?}", full.chars().take(120).collect::<String>());
    let has_line_1 = full.contains("line_1\n") || full.contains("line_1\r\n");
    println!("contains line_1: {has_line_1}");
    let has_line_60 = full.contains("line_60");
    println!("contains line_60: {has_line_60}");

    println!("\n== geometry before typing ==");
    let g0 = pane_geometry(&s).unwrap();
    println!("{g0:?}");

    // Type a partial line and check cursor advanced.
    send_keys(&s, &[Key::new("h"), Key::new("e"), Key::new("y")]).unwrap();
    sleep(Duration::from_millis(150));
    println!("\n== geometry after typing 'hey' ==");
    let g1 = pane_geometry(&s).unwrap();
    println!("{g1:?}");

    println!("\ndone");
}
