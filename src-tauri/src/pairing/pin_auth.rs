//! PIN generation, timed verification challenge, and lockout security manager

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime};

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

/// Error types occurring during PIN authentication and pairing
#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum PinAuthError {
    #[error("Pairing session for peer '{0}' not found")]
    SessionNotFound(String),
    #[error("Pairing session for peer '{0}' has expired")]
    Expired(String),
    #[error("Maximum PIN attempts exceeded for peer '{0}', session locked out")]
    MaxAttemptsExceeded(String),
    #[error("Session for peer '{0}' is already locked out")]
    LockedOut(String),
    #[error("Invalid PIN format: '{0}' (expected 4 numeric digits)")]
    InvalidPinFormat(String),
}

/// Lifecycle state of a pairing session
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PairingState {
    Pending,
    Authenticated,
    LockedOut,
    Expired,
}

/// Pairing session holding security parameters for a specific peer
#[derive(Debug, Clone)]
pub struct PairingSession {
    pub pin: String,
    pub peer_id: String,
    pub created_at: Instant,
    pub attempts: u32,
    pub max_attempts: u32,
    pub ttl: Duration,
    pub state: PairingState,
}

impl PairingSession {
    /// Create a new session with the default 300-second TTL and 3 attempts
    pub fn new(pin: impl Into<String>, peer_id: impl Into<String>) -> Self {
        Self::with_ttl(pin, peer_id, Duration::from_secs(300))
    }

    /// Create a new session with a customized TTL
    pub fn with_ttl(pin: impl Into<String>, peer_id: impl Into<String>, ttl: Duration) -> Self {
        Self {
            pin: pin.into(),
            peer_id: peer_id.into(),
            created_at: Instant::now(),
            attempts: 0,
            max_attempts: 3,
            ttl,
            state: PairingState::Pending,
        }
    }

    /// Check if the session has expired
    pub fn is_expired(&self) -> bool {
        self.state == PairingState::Expired || self.created_at.elapsed() >= self.ttl
    }

    /// Check if the session is locked out due to exceeding maximum allowed attempts
    pub fn is_locked_out(&self) -> bool {
        self.state == PairingState::LockedOut || self.attempts >= self.max_attempts
    }

    /// Number of remaining verification attempts before lockout
    pub fn remaining_attempts(&self) -> u32 {
        self.max_attempts.saturating_sub(self.attempts)
    }
}

/// OS-level entropy extraction
#[cfg(windows)]
fn get_os_random_bytes(buf: &mut [u8]) -> bool {
    #[link(name = "advapi32")]
    extern "system" {
        #[link_name = "SystemFunction036"]
        fn RtlGenRandom(buffer: *mut u8, length: u32) -> u8;
    }
    unsafe { RtlGenRandom(buf.as_mut_ptr(), buf.len() as u32) != 0 }
}

#[cfg(not(windows))]
fn get_os_random_bytes(buf: &mut [u8]) -> bool {
    use std::fs::File;
    use std::io::Read;
    if let Ok(mut f) = File::open("/dev/urandom") {
        f.read_exact(buf).is_ok()
    } else {
        false
    }
}

/// Helper function to generate an unpredictable 4-digit PIN between 1000 and 9999
/// Mixes OS CSPRNG, process ID, thread ID hash, stack address, monotonic counter, and system clock
fn generate_random_4digit() -> u32 {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let count = COUNTER.fetch_add(1, Ordering::Relaxed);

    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(987654321);

    let pid = std::process::id() as u64;
    let tid_hash = {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        std::thread::current().id().hash(&mut hasher);
        hasher.finish()
    };
    let stack_addr = &count as *const _ as usize as u64;

    let mut os_bytes = [0u8; 8];
    let os_entropy = if get_os_random_bytes(&mut os_bytes) {
        u64::from_le_bytes(os_bytes)
    } else {
        0xA5A5_5A5A_3C3C_C3C3
    };

    // Mix multi-source entropy
    let mut z = os_entropy
        ^ nanos.wrapping_add(count.wrapping_mul(0x9E3779B97F4A7C15))
        ^ pid.rotate_left(16)
        ^ tid_hash.rotate_left(32)
        ^ stack_addr.rotate_left(48);

    // SplitMix64 mixing rounds
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
    z = z ^ (z >> 31);

    1000 + ((z % 9000) as u32)
}

/// Authentication and pairing manager enforcing PIN generation, timeout, and lockout
pub struct PinAuthManager {
    sessions: HashMap<String, PairingSession>,
    default_ttl: Duration,
    default_max_attempts: u32,
}

impl Default for PinAuthManager {
    fn default() -> Self {
        Self::new()
    }
}

impl PinAuthManager {
    /// Create a new PIN authentication manager with default 300s expiration and 3-attempt limit
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
            default_ttl: Duration::from_secs(300),
            default_max_attempts: 3,
        }
    }

    /// Create manager with customized default TTL
    pub fn with_ttl(ttl: Duration) -> Self {
        Self {
            sessions: HashMap::new(),
            default_ttl: ttl,
            default_max_attempts: 3,
        }
    }

    /// Create manager with customized default TTL and attempt limits
    pub fn with_ttl_and_max_attempts(ttl: Duration, max_attempts: u32) -> Self {
        Self {
            sessions: HashMap::new(),
            default_ttl: ttl,
            default_max_attempts: max_attempts,
        }
    }

    /// Generate a cryptographically random 4-digit PIN ("1000" to "9999") and initialize pairing session
    pub fn generate_pin(&mut self, peer_id: &str) -> String {
        let pin_val = generate_random_4digit();
        let pin = format!("{:04}", pin_val);
        let mut session = PairingSession::with_ttl(&pin, peer_id, self.default_ttl);
        session.max_attempts = self.default_max_attempts;
        self.sessions.insert(peer_id.to_string(), session);
        pin
    }

    /// Verify a candidate PIN against the active pairing session
    ///
    /// Returns:
    /// - `Ok(true)` if the candidate PIN matches and session is authenticated (or already authenticated).
    /// - `Ok(false)` if candidate PIN does not match and attempts are within threshold.
    /// - `Err(...)` if format is invalid, session doesn't exist, is expired, or is locked out.
    pub fn verify_pin(&mut self, peer_id: &str, candidate_pin: &str) -> Result<bool> {
        // Enforce 4-digit numeric format
        if candidate_pin.len() != 4 || !candidate_pin.chars().all(|c| c.is_ascii_digit()) {
            bail!(PinAuthError::InvalidPinFormat(candidate_pin.to_string()));
        }

        let session = self.sessions.get_mut(peer_id).ok_or_else(|| {
            PinAuthError::SessionNotFound(peer_id.to_string())
        })?;

        // Idempotent State Machine: if session is already Authenticated, return Ok(true)
        // without mutating attempt count or risking subsequent lockout
        if session.state == PairingState::Authenticated {
            return Ok(true);
        }

        // Enforce lockout check
        if session.is_locked_out() {
            bail!(PinAuthError::MaxAttemptsExceeded(peer_id.to_string()));
        }

        // Enforce 300-second expiration check
        if session.is_expired() {
            session.state = PairingState::Expired;
            bail!(PinAuthError::Expired(peer_id.to_string()));
        }

        if candidate_pin == session.pin {
            session.state = PairingState::Authenticated;
            Ok(true)
        } else {
            session.attempts += 1;
            if session.attempts >= session.max_attempts {
                session.state = PairingState::LockedOut;
            }
            Ok(false)
        }
    }

    /// Get active session reference for a peer
    pub fn get_session(&self, peer_id: &str) -> Option<&PairingSession> {
        self.sessions.get(peer_id)
    }

    /// Check if a peer's session is locked out
    pub fn is_locked_out(&self, peer_id: &str) -> bool {
        self.sessions.get(peer_id).map_or(false, |s| s.is_locked_out())
    }

    /// Check if a peer's session is expired
    pub fn is_expired(&self, peer_id: &str) -> bool {
        self.sessions.get(peer_id).map_or(false, |s| s.is_expired())
    }

    /// Get remaining verification attempts for a peer
    pub fn remaining_attempts(&self, peer_id: &str) -> Option<u32> {
        self.sessions.get(peer_id).map(|s| s.remaining_attempts())
    }

    /// Expire session explicitly for testing or timeout simulation
    pub fn expire_session_for_test(&mut self, peer_id: &str) {
        if let Some(session) = self.sessions.get_mut(peer_id) {
            session.state = PairingState::Expired;
        }
    }

    /// Clear pairing session for a peer
    pub fn clear_session(&mut self, peer_id: &str) {
        self.sessions.remove(peer_id);
    }

    /// Return total active sessions
    pub fn active_session_count(&self) -> usize {
        self.sessions.len()
    }

    /// Clean up expired sessions from memory
    pub fn cleanup_expired_sessions(&mut self) -> usize {
        let initial = self.sessions.len();
        self.sessions.retain(|_, s| !s.is_expired());
        initial - self.sessions.len()
    }
}
