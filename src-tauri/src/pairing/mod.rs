//! Connection security & PIN pairing

pub mod pin_auth;

pub use pin_auth::{
    IncomingRequest, PairingSession, PairingState, PinAuthError, PinAuthManager,
};
