// Manual validation for e3-harness: real subprocess JSON-RPC roundtrip
// over stdio. The example spawns itself as a child process that serves,
// and the parent acts as the client.
//
// Run: cargo run -p tmuxwright-rpc --example e3_harness

use std::io::{BufReader, Write};
use std::process::{Command, Stdio};

use serde_json::Value;

use tmuxwright_rpc::client::Client;
use tmuxwright_rpc::schema::{
    ActionKind, ActionParams, ActionResult, Capability, HandshakeResult, LocateParams,
    LocateResult, NodeRef, RegionWire, SelectorWire, PROTOCOL_VERSION,
};
use tmuxwright_rpc::server::{serve, Handler};
use tmuxwright_rpc::RpcError;

struct DemoAdapter {
    stop: bool,
}

impl Handler for DemoAdapter {
    fn handshake(&mut self, _p: Value) -> Result<Value, RpcError> {
        Ok(serde_json::to_value(HandshakeResult {
            name: "demo".into(),
            version: "0.0.1".into(),
            protocol: PROTOCOL_VERSION.into(),
            capabilities: vec![
                Capability::KeyInput,
                Capability::MouseInput,
                Capability::WidgetTree,
            ],
        })
        .unwrap())
    }
    fn locate(&mut self, _p: Value) -> Result<Value, RpcError> {
        Ok(serde_json::to_value(LocateResult {
            nodes: vec![NodeRef {
                node_id: "btn-save".into(),
                region: Some(RegionWire {
                    x: 10,
                    y: 5,
                    w: 8,
                    h: 1,
                }),
                role: Some("button".into()),
                name: Some("Save".into()),
            }],
        })
        .unwrap())
    }
    fn action(&mut self, _p: Value) -> Result<Value, RpcError> {
        Ok(serde_json::to_value(ActionResult { applied: true }).unwrap())
    }
    fn shutdown(&mut self, _p: Value) -> Result<Value, RpcError> {
        self.stop = true;
        Ok(Value::Null)
    }
    fn should_stop(&self) -> bool {
        self.stop
    }
}

fn run_server() {
    let mut h = DemoAdapter { stop: false };
    let stdin = std::io::stdin();
    let mut r = BufReader::new(stdin.lock());
    let stdout = std::io::stdout();
    let mut w = stdout.lock();
    serve(&mut h, &mut r, &mut w).expect("serve failed");
    w.flush().ok();
}

fn run_client() {
    let exe = std::env::current_exe().expect("exe");
    let mut child = Command::new(exe)
        .env("TMW_E3_MODE", "server")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("spawn");
    let stdin = child.stdin.take().unwrap();
    let stdout = BufReader::new(child.stdout.take().unwrap());
    let mut client = Client::new(stdout, stdin);

    let hr = client.handshake("tmuxwright", "0.0.0").expect("handshake");
    println!(
        "handshake -> {} {} protocol={} caps={:?}",
        hr.name, hr.version, hr.protocol, hr.capabilities
    );

    let lr = client
        .locate(&LocateParams {
            selector: SelectorWire::Role {
                role: "button".into(),
                name: Some("Save".into()),
            },
        })
        .expect("locate");
    println!(
        "locate    -> {} nodes; first={:?}",
        lr.nodes.len(),
        lr.nodes[0]
    );

    let ar = client
        .action(&ActionParams {
            node_id: lr.nodes[0].node_id.clone(),
            action: ActionKind::Click,
        })
        .expect("action");
    println!("action    -> applied={}", ar.applied);

    let _ = client.shutdown().expect("shutdown");
    println!("shutdown  -> ok");

    // Drive the child to exit by closing stdin/stdout handles.
    drop(client);
    let status = child.wait().expect("wait");
    println!("child exit: {status}");
    assert!(status.success());
}

fn main() {
    if std::env::var("TMW_E3_MODE").as_deref() == Ok("server") {
        run_server();
    } else {
        run_client();
    }
}
