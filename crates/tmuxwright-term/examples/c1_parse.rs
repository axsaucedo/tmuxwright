// Manual validation for c1-vte-parse.
// Feeds a small ANSI sample into the parser and prints the grid,
// cursor, and selected attributes.
//
// Run: cargo run -p tmuxwright-term --example c1_parse

use tmuxwright_term::{Color, Parser};

fn main() {
    let mut p = Parser::new(30, 5);
    p.feed(b"hello\r\n");
    p.feed(b"\x1b[1;4;31mBOLD-RED-UNDER\x1b[0m plain\r\n");

    // Before any cursor-jump, validate the attrs of the styled row.
    let g = p.grid();
    let c = g.cell(0, 1).expect("cell");
    println!("cell (0,1) char={:?} attrs={:?}", c.ch, c.attrs);
    assert_eq!(c.ch, 'B');
    assert!(c.attrs.bold);
    assert!(c.attrs.underline);
    assert_eq!(c.attrs.fg, Color::Indexed(1));

    let plain_x = u16::try_from("BOLD-RED-UNDER ".len()).unwrap();
    let c2 = g.cell(plain_x, 1).expect("plain cell");
    println!("cell (plain) char={:?} attrs={:?}", c2.ch, c2.attrs);
    assert!(!c2.attrs.bold);
    assert_eq!(c2.attrs.fg, Color::Default);

    // Now exercise cursor control + erase-in-line.
    p.feed(b"tail\x1b[3;1Hline three\x1b[K extra");

    let g = p.grid();
    println!(
        "\ngrid {}x{} cursor={:?}",
        g.width(),
        g.height(),
        g.cursor()
    );
    for y in 0..g.height() {
        println!("row {y}: {:?}", g.row_text(y));
    }
    // Row 2 (0-based) now has "line three extra" because "\x1b[K" erased
    // to EOL from cursor before " extra" was written.
    assert!(
        g.row_text(2).starts_with("line three"),
        "got: {:?}",
        g.row_text(2)
    );

    println!("\ndone");
}
