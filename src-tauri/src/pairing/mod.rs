//! Connection security & PIN pairing

pub mod pin_auth;

pub use pin_auth::{PairingSession, PairingState, PinAuthError, PinAuthManager};
