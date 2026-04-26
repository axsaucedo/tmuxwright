//! Terminal assertions backed by parsed snapshots.

use tmuxwright_core::snapshot::Snapshot;
use tmuxwright_term::locator::TextLocator;

use crate::protocol::AssertTextResult;

pub fn assert_text(snap: &Snapshot, contains: &str) -> AssertTextResult {
    let loc = TextLocator::new(contains);
    if let Some(hit) = loc.first(&snap.grid) {
        let r = hit.region;
        AssertTextResult {
            matched: true,
            region: Some([r.x, r.y, r.width, r.height]),
        }
    } else {
        AssertTextResult {
            matched: false,
            region: None,
        }
    }
}
