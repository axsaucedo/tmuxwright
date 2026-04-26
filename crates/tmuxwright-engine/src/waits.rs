//! Wait helpers for daemon methods.

use std::time::{Duration, Instant};

use tmuxwright_core::snapshot::Snapshot;
use tmuxwright_rpc::RpcError;

use crate::protocol::WaitStableResult;

pub fn wait_stable(
    timeout_ms: u64,
    quiet_ms: u64,
    mut sample: impl FnMut() -> Result<Snapshot, RpcError>,
) -> Result<WaitStableResult, RpcError> {
    let deadline = Instant::now() + Duration::from_millis(timeout_ms);
    let quiet = Duration::from_millis(quiet_ms);
    let mut last_hash = sample()?.hash;
    let mut unchanged_since = Instant::now();
    loop {
        std::thread::sleep(Duration::from_millis(50));
        let snap = sample()?;
        if snap.hash != last_hash {
            last_hash = snap.hash;
            unchanged_since = Instant::now();
        }
        if unchanged_since.elapsed() >= quiet {
            return Ok(WaitStableResult {
                status: "stable",
                hash: last_hash.hex(),
            });
        }
        if Instant::now() >= deadline {
            return Ok(WaitStableResult {
                status: "timeout",
                hash: last_hash.hex(),
            });
        }
    }
}
