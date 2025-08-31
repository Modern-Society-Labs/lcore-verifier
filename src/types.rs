//! Type definitions for the verifier service

use serde::{Deserialize, Serialize};

/// Proof request from Cartesi notice
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProofRequest {
    pub device_id: String,
    pub proof_type: String,
    pub receipt_url: String,
    pub expected_image_id: String,
    pub epoch_index: u64,
    pub input_index: u64,
}

/// Verified receipt to be submitted to InputBox
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifiedReceipt {
    /// Device ID that submitted the original proof
    pub device_id: String,
    
    /// Type of proof (iot_validation, iot_privacy, iot_compute)
    pub proof_type: String,
    
    /// Keccak256 hash of the full RISC Zero receipt
    pub receipt_hash: String,
    
    /// Expected RISC Zero guest program image ID
    pub image_id: String,
    
    /// Keccak256 hash of the journal data
    pub journal_hash: String,
    
    /// Cartesi epoch index
    pub epoch_index: u64,
    
    /// Input index within the epoch
    pub input_index: u64,
    
    /// ECDSA signature over keccak256 of all above fields
    pub signature: String,
    
    /// Optional: Unix timestamp of verification
    pub timestamp: Option<u64>,
    
    /// Optional: Address of the verifier who signed this receipt
    pub verifier_address: Option<String>,
}

/// GraphQL notice data
#[derive(Debug, Clone, Deserialize)]
pub struct Notice {
    pub index: String,
    pub input_index: String,
    pub payload: String,
}

/// GraphQL input data
#[derive(Debug, Clone, Deserialize)]
pub struct Input {
    pub index: String,
    pub timestamp: String,
}

/// InputBox payload format
#[derive(Debug, Clone, Serialize)]
pub struct InputBoxPayload {
    pub address: String,
    pub payload: String,
}
