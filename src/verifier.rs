// RISC Zero Proof Verification Logic

use anyhow::{anyhow, Result};
use risc0_zkvm::{Receipt, ReceiptClaim};
use sha3::{Digest, Keccak256};
use hex;
use log::{info, debug};

#[derive(Debug, Clone)]
pub struct VerificationResult {
    pub is_valid: bool,
    pub receipt_hash: String,
    pub image_id: String,
    pub journal_hash: String,
    pub exit_code: u8,
    pub error_message: Option<String>,
}

pub struct ProofVerifier {
    allowed_image_ids: Vec<String>,
}

impl ProofVerifier {
    pub fn new(allowed_image_ids: Vec<String>) -> Self {
        info!("Initialized RISC Zero verifier with {} allowed image IDs", 
              allowed_image_ids.len());
        Self { allowed_image_ids }
    }
    
    /// Verify a RISC Zero proof receipt
    pub fn verify_receipt(
        &self,
        receipt_bytes: &[u8],
        expected_image_id: &str,
    ) -> Result<VerificationResult> {
        // Compute receipt hash
        let mut hasher = Keccak256::new();
        hasher.update(receipt_bytes);
        let receipt_hash = format!("0x{}", hex::encode(hasher.finalize()));
        
        debug!("Verifying receipt with hash: {}", receipt_hash);
        
        // Deserialize the receipt
        let receipt: Receipt = bincode::deserialize(receipt_bytes)
            .map_err(|e| anyhow!("Failed to deserialize receipt: {}", e))?;
        
        // Extract claim information
        let claim = receipt.inner.claim()?;
        let image_id = format!("0x{}", hex::encode(&claim.pre_state_digest));
        let journal_hash = format!("0x{}", hex::encode(
            Keccak256::digest(&receipt.journal).as_slice()
        ));
        let exit_code = claim.exit_code;
        
        // Check if image ID matches expected
        if image_id != expected_image_id {
            return Ok(VerificationResult {
                is_valid: false,
                receipt_hash,
                image_id,
                journal_hash,
                exit_code,
                error_message: Some(format!(
                    "Image ID mismatch: expected {}, got {}",
                    expected_image_id, image_id
                )),
            });
        }
        
        // Check if image ID is in allowed list
        if !self.allowed_image_ids.is_empty() && !self.allowed_image_ids.contains(&image_id) {
            return Ok(VerificationResult {
                is_valid: false,
                receipt_hash,
                image_id,
                journal_hash,
                exit_code,
                error_message: Some(format!("Image ID {} not in allowed list", image_id)),
            });
        }
        
        // Verify the receipt cryptographically
        match receipt.verify() {
            Ok(_) => {
                info!("Receipt verified successfully: image_id={}, exit_code={}", 
                      image_id, exit_code);
                      
                Ok(VerificationResult {
                    is_valid: exit_code == 0,
                    receipt_hash,
                    image_id,
                    journal_hash,
                    exit_code,
                    error_message: if exit_code != 0 {
                        Some(format!("Guest program exited with code {}", exit_code))
                    } else {
                        None
                    },
                })
            }
            Err(e) => {
                Ok(VerificationResult {
                    is_valid: false,
                    receipt_hash,
                    image_id,
                    journal_hash,
                    exit_code,
                    error_message: Some(format!("Receipt verification failed: {}", e)),
                })
            }
        }
    }
}
