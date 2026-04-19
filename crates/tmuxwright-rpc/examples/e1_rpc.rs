// Manual validation for e1-rpc.
// Run: cargo run -p tmuxwright-rpc --example e1_rpc

use std::io::Cursor;

use tmuxwright_rpc::framing::{read_message, write_message};
use tmuxwright_rpc::{Id, Notification, Request, Response, RpcError};

fn main() {
    let req = Request::new(
        Id::Num(1),
        "adapter.handshake",
        Some(serde_json::json!({"client":"tmuxwright","version":"0.0.0"})),
    );
    let req_json = serde_json::to_string(&req).unwrap();
    println!("request:  {req_json}");

    let resp_ok = Response::ok(
        Id::Num(1),
        serde_json::json!({
            "name":"textual",
            "version":"0.1.0",
            "protocol":"1",
            "capabilities":["key_input","widget_tree"]
        }),
    );
    let resp_err = Response::err(
        Id::Num(2),
        RpcError::new(RpcError::METHOD_NOT_FOUND, "unknown method: widget.tree")
            .with_data(serde_json::json!({"known":["adapter.handshake","action.dispatch"]})),
    );
    println!("response: {}", serde_json::to_string(&resp_ok).unwrap());
    println!("error   : {}", serde_json::to_string(&resp_err).unwrap());

    let note = Notification::new("progress", Some(serde_json::json!({"step":2})));
    println!("notify  : {}", serde_json::to_string(&note).unwrap());

    let mut buf = Vec::new();
    write_message(&mut buf, &req_json).unwrap();
    write_message(&mut buf, &serde_json::to_string(&resp_ok).unwrap()).unwrap();
    println!("\nframed bytes ({}): {buf:?}", buf.len());

    let mut cur = Cursor::new(buf);
    let a = read_message(&mut cur).unwrap().unwrap();
    let b = read_message(&mut cur).unwrap().unwrap();
    println!("decoded [0]: {a}");
    println!("decoded [1]: {b}");
    let c = read_message(&mut cur).unwrap();
    println!("decoded [2]: {c:?}");
    assert!(c.is_none());

    println!("\ndone");
}
