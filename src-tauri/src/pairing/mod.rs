//! Connection security & PIN pairing

#[derive(Debug, Clone)]
pub struct PairingSession {
    pub pin: String,
    pub peer_id: String,
    pub created_at: std::time::Instant,
}
