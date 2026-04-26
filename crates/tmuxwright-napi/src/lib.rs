//! Future napi-rs bindings for Tmuxwright.
//!
//! Terminal-mode v1 uses the `tmuxwright-engine` stdio daemon as the
//! TypeScript boundary. Native Node bindings are intentionally deferred
//! until the daemon-backed SDK is stable and there is evidence that
//! startup or distribution costs justify the added build complexity.
//!
//! This crate is not an active workspace member for v1.
