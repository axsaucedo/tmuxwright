// Manual validation for b3-input-inject.
//
// Validates:
//   - send_keys emits symbolic keys (types "hello" char-by-char).
//   - type_text routes literal text via load-buffer/paste-buffer so
//     that bytes that *look* like key names land verbatim.
//   - send_mouse produces the SGR escape sequence and tmux accepts it.
//
// The session runs `cat`, which simply echoes stdin to stdout. After
// each input step we capture the pane and print what it contains.
//
// Run: cargo run -p tmuxwright-tmux --example b3_input

use std::thread::sleep;
use std::time::Duration;

use tmuxwright_tmux::{
    detect, encode_mouse_sgr, send_keys, send_mouse, type_text, Key, MouseButton, MouseEvent,
    Session, SessionOptions,
};

fn capture(session: &Session) -> String {
    let target = session.primary_pane_target();
    let out = session
        .tmux_cmd(&["capture-pane", "-t", &target, "-p"])
        .expect("capture-pane");
    String::from_utf8_lossy(&out.stdout).trim_end().to_string()
}

fn main() {
    let tmux = detect().expect("tmux required");
    let opts = SessionOptions {
        width: 80,
        height: 24,
        command: vec!["cat".into()],
    };
    let session = Session::create(tmux, &opts).expect("create session");
    sleep(Duration::from_millis(150));

    println!("== send_keys: h e l l o Enter ==");
    let keys: Vec<Key> = ["h", "e", "l", "l", "o", "Enter"]
        .iter()
        .map(|k| Key::new(*k))
        .collect();
    send_keys(&session, &keys).expect("send_keys");
    sleep(Duration::from_millis(150));
    println!("{}", capture(&session));

    println!("\n== type_text: 'Enter means Enter' + Enter ==");
    type_text(&session, "Enter means Enter\n").expect("type_text");
    sleep(Duration::from_millis(150));
    println!("{}", capture(&session));

    println!("\n== encode_mouse_sgr samples ==");
    for (btn, ev, x, y) in [
        (MouseButton::Left, MouseEvent::Press, 10, 5),
        (MouseButton::Left, MouseEvent::Release, 10, 5),
        (MouseButton::WheelUp, MouseEvent::Press, 1, 1),
    ] {
        let s = encode_mouse_sgr(btn, ev, x, y);
        println!("  {btn:?} {ev:?} {x},{y} => {s:?}");
    }

    println!("\n== send_mouse: left-press at 10,5 (tmux must accept it) ==");
    send_mouse(&session, MouseButton::Left, MouseEvent::Press, 10, 5).expect("send_mouse");
    send_mouse(&session, MouseButton::Left, MouseEvent::Release, 10, 5).expect("send_mouse");
    println!("(no crash => tmux accepted the SGR hex bytes)");

    println!("\ndone");
}
