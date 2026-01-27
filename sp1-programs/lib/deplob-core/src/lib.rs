//! Shared types and utilities for DePLOB SP1 programs

use serde::{Deserialize, Serialize};

/// Commitment structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Commitment {
    pub hash: [u8; 32],
}

/// Nullifier structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Nullifier {
    pub hash: [u8; 32],
}
