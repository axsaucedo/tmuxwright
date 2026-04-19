// Manual validation for c2-stable-hash.
// Run: cargo run -p tmuxwright-term --example c2_hash

use tmuxwright_term::{hash_grid, Parser};

fn parse(w: u16, h: u16, s: &[u8]) -> tmuxwright_term::Grid {
    let mut p = Parser::new(w, h);
    p.feed(s);
    p.into_grid()
}

fn main() {
    let hello_plain = hash_grid(&parse(20, 4, b"hello\r\nworld"));
    let hello_plain_again = hash_grid(&parse(20, 4, b"hello\r\nworld"));
    let hello_with_bang = hash_grid(&parse(20, 4, b"hello\r\nworld!"));
    let hello_bold = hash_grid(&parse(20, 4, b"\x1b[1mhello\r\nworld"));
    let hello_via_cup = hash_grid(&parse(20, 4, b"\x1b[1;1Hhello\r\nworld"));

    println!("a = {hello_plain}");
    println!("b = {hello_plain_again}");
    println!("c = {hello_with_bang}");
    println!("d = {hello_bold}");
    println!("e = {hello_via_cup}");
    println!("short(a) = {}", hello_plain.short());

    assert_eq!(
        hello_plain, hello_plain_again,
        "identical inputs must hash identically"
    );
    assert_ne!(
        hello_plain, hello_with_bang,
        "different text must change hash"
    );
    assert_ne!(hello_plain, hello_bold, "bold attr must change hash");
    assert_eq!(
        hello_plain, hello_via_cup,
        "idempotent redraw via CUP must match"
    );

    println!("\ndone");
}
