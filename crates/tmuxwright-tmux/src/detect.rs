//! tmux binary discovery and version gating.
//!
//! Tmuxwright targets `tmux >= 3.3` because several capture and
//! control-mode improvements we rely on landed in 3.x. This module
//! centralizes the logic for locating the binary and deciding whether
//! its version is acceptable, so every other tmux-facing function in
//! the crate can start from a validated `Tmux` handle.

use std::cmp::Ordering;
use std::path::{Path, PathBuf};
use std::process::Command;

use thiserror::Error;

/// Minimum tmux version Tmuxwright supports.
pub const MIN_TMUX_VERSION: Version = Version { major: 3, minor: 3 };

/// Errors produced while locating or validating tmux.
#[derive(Debug, Error)]
pub enum DetectError {
    #[error("tmux binary not found on PATH; install tmux >= {MIN_TMUX_VERSION} and try again")]
    NotFound,
    #[error("failed to execute tmux at {path}: {source}")]
    Exec {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("tmux at {path} returned non-zero status when asked for -V")]
    NonZeroStatus { path: PathBuf },
    #[error("could not parse tmux version banner: {raw:?}")]
    ParseVersion { raw: String },
    #[error("tmux at {path} reports version {found}, but Tmuxwright requires >= {required}")]
    TooOld {
        path: PathBuf,
        found: Version,
        required: Version,
    },
}

/// A parsed tmux version. Tmuxwright only looks at major/minor because
/// tmux's own versioning ("3.4", "3.3a") does not use a patch component.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Version {
    pub major: u16,
    pub minor: u16,
}

impl Version {
    #[must_use]
    pub const fn new(major: u16, minor: u16) -> Self {
        Self { major, minor }
    }
}

impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}", self.major, self.minor)
    }
}

impl Ord for Version {
    fn cmp(&self, other: &Self) -> Ordering {
        (self.major, self.minor).cmp(&(other.major, other.minor))
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// A validated tmux binary that meets Tmuxwright's minimum version.
#[derive(Debug, Clone)]
pub struct Tmux {
    path: PathBuf,
    version: Version,
}

impl Tmux {
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    #[must_use]
    pub fn version(&self) -> Version {
        self.version
    }
}

/// Locate tmux on PATH, invoke `tmux -V`, parse the version, and enforce
/// the minimum-version policy.
pub fn detect() -> Result<Tmux, DetectError> {
    let path = which::which("tmux").map_err(|_| DetectError::NotFound)?;
    detect_at(&path)
}

/// Like `detect` but uses a specific tmux binary path. Useful for tests
/// and for honoring an explicit `TMUX_BIN` override in higher layers.
pub fn detect_at(path: &Path) -> Result<Tmux, DetectError> {
    let output = Command::new(path)
        .arg("-V")
        .output()
        .map_err(|source| DetectError::Exec {
            path: path.to_path_buf(),
            source,
        })?;

    if !output.status.success() {
        return Err(DetectError::NonZeroStatus {
            path: path.to_path_buf(),
        });
    }

    let banner = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let version = parse_version_banner(&banner).ok_or(DetectError::ParseVersion {
        raw: banner.clone(),
    })?;

    if version < MIN_TMUX_VERSION {
        return Err(DetectError::TooOld {
            path: path.to_path_buf(),
            found: version,
            required: MIN_TMUX_VERSION,
        });
    }

    Ok(Tmux {
        path: path.to_path_buf(),
        version,
    })
}

/// Parse a `tmux -V` banner such as "tmux 3.4", "tmux 3.3a", or
/// "tmux next-3.5" into a `Version`. Suffixes after the numeric
/// minor component (e.g. "a" or "-rc") are ignored because tmux uses
/// them for pre-/post-release builds that still behave as their base
/// minor version for our purposes.
#[must_use]
pub fn parse_version_banner(banner: &str) -> Option<Version> {
    let trimmed = banner.trim();
    let after_prefix = trimmed.strip_prefix("tmux ")?.trim_start_matches("next-");
    let mut iter = after_prefix.split('.');
    let major: u16 = iter.next()?.parse().ok()?;
    let minor_raw = iter.next()?;
    let minor_digits: String = minor_raw.chars().take_while(char::is_ascii_digit).collect();
    if minor_digits.is_empty() {
        return None;
    }
    let minor: u16 = minor_digits.parse().ok()?;
    Some(Version { major, minor })
}
