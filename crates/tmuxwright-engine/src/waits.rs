//! Wait helpers for daemon methods.

use std::time::{Duration, Instant};

use tmuxwright_core::snapshot::Snapshot;
use tmuxwright_rpc::RpcError;
use tmuxwright_term::locator::TextLocator;
use tmuxwright_term::{MonotonicClock, Stability, StabilityConfig, Status};

use crate::protocol::{WaitHashResult, WaitStableResult, WaitTextResult};

pub fn wait_stable(
    timeout_ms: u64,
    quiet_ms: u64,
    mut sample: impl FnMut() -> Result<Snapshot, RpcError>,
) -> Result<WaitStableResult, RpcError> {
    let mut stability = Stability::new(
        MonotonicClock,
        StabilityConfig {
            quiet_for: Duration::from_millis(quiet_ms),
            timeout: Duration::from_millis(timeout_ms),
        },
    );
    loop {
        let hash = sample()?.hash;
        match stability.observe(hash) {
            Status::Stable => {
                return Ok(WaitStableResult {
                    status: "stable",
                    hash: hash.hex(),
                });
            }
            Status::Timeout => {
                return Ok(WaitStableResult {
                    status: "timeout",
                    hash: hash.hex(),
                });
            }
            Status::Changing => std::thread::sleep(Duration::from_millis(50)),
        }
    }
}

pub fn wait_text(
    contains: &str,
    timeout_ms: u64,
    mut sample: impl FnMut() -> Result<Snapshot, RpcError>,
) -> Result<WaitTextResult, RpcError> {
    let deadline = Instant::now() + Duration::from_millis(timeout_ms);
    let loc = TextLocator::new(contains);
    loop {
        let snap = sample()?;
        if let Some(hit) = loc.first(&snap.grid) {
            let r = hit.region;
            return Ok(WaitTextResult {
                status: "found",
                matched: true,
                hash: snap.hash.hex(),
                region: Some([r.x, r.y, r.width, r.height]),
            });
        }
        if Instant::now() >= deadline {
            return Ok(WaitTextResult {
                status: "timeout",
                matched: false,
                hash: snap.hash.hex(),
                region: None,
            });
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

pub fn wait_hash(
    expected: &str,
    timeout_ms: u64,
    mut sample: impl FnMut() -> Result<Snapshot, RpcError>,
) -> Result<WaitHashResult, RpcError> {
    let deadline = Instant::now() + Duration::from_millis(timeout_ms);
    loop {
        let hash = sample()?.hash.hex();
        if hash == expected {
            return Ok(WaitHashResult {
                status: "found",
                hash,
            });
        }
        if Instant::now() >= deadline {
            return Ok(WaitHashResult {
                status: "timeout",
                hash,
            });
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}
