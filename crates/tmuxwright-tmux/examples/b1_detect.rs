// Manual validation for b1-tmux-detect.
//
// What this validates:
//   - `detect()` finds tmux on the developer's PATH.
//   - The reported version parses correctly from the tmux -V banner.
//   - `parse_version_banner` handles the quirky forms tmux actually
//     emits ("tmux 3.4", "tmux 3.3a", "tmux next-3.5").
//   - `detect_at` surfaces the right error when pointed at a bogus path.
//
// How to run (from the repo root):
//   cargo run -p tmuxwright-tmux --example b1_detect
//
// Expected observable outcome:
//   - Line 1 prints the local tmux path and version.
//   - Parse cases print PASS for each banner variant.
//   - The bogus-path case prints a `NotFound`/`Exec` error without panicking.

use std::path::PathBuf;

use tmuxwright_tmux::{detect, detect_at, parse_version_banner, Version};

fn main() {
    println!("== detect() ==");
    match detect() {
        Ok(tmux) => println!(
            "  found tmux {} at {}",
            tmux.version(),
            tmux.path().display()
        ),
        Err(err) => println!("  ERROR: {err}"),
    }

    println!("\n== parse_version_banner ==");
    let cases = [
        ("tmux 3.4", Some(Version::new(3, 4))),
        ("tmux 3.3a", Some(Version::new(3, 3))),
        ("tmux next-3.5", Some(Version::new(3, 5))),
        ("tmux 2.9", Some(Version::new(2, 9))),
        ("garbage output", None),
    ];
    for (raw, expected) in cases {
        let got = parse_version_banner(raw);
        let ok = got == expected;
        println!(
            "  {}  {:?} -> {:?}",
            if ok { "PASS" } else { "FAIL" },
            raw,
            got
        );
    }

    println!("\n== detect_at(bogus) ==");
    let bogus = PathBuf::from("/definitely/not/a/real/tmux-binary");
    match detect_at(&bogus) {
        Ok(_) => println!("  UNEXPECTED: detect_at succeeded on a bogus path"),
        Err(err) => println!("  got expected error: {err}"),
    }
}
