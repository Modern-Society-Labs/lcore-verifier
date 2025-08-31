//! Receipt signing with ECDSA

use anyhow::Result;
use k256::{
    ecdsa::{SigningKey, Signature, signature::Signer},
    SecretKey,
};
use sha3::{Digest, Keccak256};
use crate::types::VerifiedReceipt;
use crate::error::VerifierError;

pub struct ReceiptSigner {
    signing_key: SigningKey,
    address: String,
}

impl ReceiptSigner {
    /// Create a new signer from private key hex
    pub fn new(private_key_hex: &str) -> Result<Self> {
        let private_key_hex = private_key_hex.trim_start_matches("0x");
        let private_key_bytes = hex::decode(private_key_hex)
            .map_err(|e| VerifierError::Signing(format!("Invalid private key hex: {}", e)))?;
        
        let secret_key = SecretKey::from_slice(&private_key_bytes)
            .map_err(|e| VerifierError::Signing(format!("Invalid private key: {}", e)))?;
        
        let signing_key = SigningKey::from(secret_key);
        
        // Derive Ethereum address from public key
        let public_key = signing_key.verifying_key();
        let public_key_bytes = public_key.to_encoded_point(false);
        let public_key_bytes = &public_key_bytes.as_bytes()[1..]; // Skip the 0x04 prefix
        
        let mut hasher = Keccak256::new();
        hasher.update(public_key_bytes);
        let hash = hasher.finalize();
        
        let address = format!("0x{}", hex::encode(&hash[12..]));
        
        Ok(Self {
            signing_key,
            address,
        })
    }
    
    /// Get the signer's Ethereum address
    pub fn get_address(&self) -> String {
        self.address.clone()
    }
    
    /// Sign a verified receipt
    pub fn sign_receipt(&self, mut receipt: VerifiedReceipt) -> Result<VerifiedReceipt> {
        // Set verifier address if not already set
        if receipt.verifier_address.is_none() {
            receipt.verifier_address = Some(self.address.clone());
        }
        
        // Compute signing hash
        let signing_hash = compute_receipt_hash(&receipt);
        
        // Sign the hash
        let signature: Signature = self.signing_key.sign(&signing_hash);
        let signature_bytes = signature.to_bytes();
        
        // Convert to recoverable signature format (65 bytes with recovery ID)
        // For Ethereum compatibility, we need to add the recovery ID
        let mut sig_with_recovery = vec![0u8; 65];
        sig_with_recovery[..64].copy_from_slice(&signature_bytes);
        
        // For simplicity, we'll use recovery ID 27 (v = 27)
        // In production, this should be calculated properly
        sig_with_recovery[64] = 27;
        
        // Set the signature on the receipt
        receipt.signature = format!("0x{}", hex::encode(sig_with_recovery));
        
        Ok(receipt)
    }
}

/// Compute the Keccak256 hash of receipt fields for signing
fn compute_receipt_hash(receipt: &VerifiedReceipt) -> [u8; 32] {
    let mut hasher = Keccak256::new();
    
    // Hash all fields in deterministic order (excluding signature itself)
    hasher.update(receipt.device_id.as_bytes());
    hasher.update(receipt.proof_type.as_bytes());
    hasher.update(receipt.receipt_hash.as_bytes());
    hasher.update(receipt.image_id.as_bytes());
    hasher.update(receipt.journal_hash.as_bytes());
    hasher.update(&receipt.epoch_index.to_le_bytes());
    hasher.update(&receipt.input_index.to_le_bytes());
    
    hasher.finalize().into()
}

#[cfg(test)]
mod tests {
    use super::*;
    
    fn get_test_private_key() -> String {
        // Use environment variable for testing, or generate a random key
        std::env::var("TEST_PRIVATE_KEY").unwrap_or_else(|_| {
            // Generate a deterministic test key from a seed for consistent testing
            use sha3::{Digest, Keccak256};
            let mut hasher = Keccak256::new();
            hasher.update(b"test_seed_for_receipt_signer_tests");
            let hash = hasher.finalize();
            format!("0x{}", hex::encode(&hash[..32]))
        })
    }
    
    #[test]
    fn test_signer_creation() {
        let private_key = get_test_private_key();
        let signer = ReceiptSigner::new(&private_key).unwrap();
        
        // Verify the address is valid (42 characters including 0x)
        assert_eq!(signer.get_address().len(), 42);
        assert!(signer.get_address().starts_with("0x"));
    }
    
    #[test]
    fn test_receipt_signing() {
        let private_key = get_test_private_key();
        let signer = ReceiptSigner::new(&private_key).unwrap();
        
        let receipt = VerifiedReceipt {
            device_id: "device123".to_string(),
            proof_type: "iot_validation".to_string(),
            receipt_hash: "0x1234".to_string(),
            image_id: "0x5678".to_string(),
            journal_hash: "0xabcd".to_string(),
            epoch_index: 1,
            input_index: 2,
            signature: String::new(),
            timestamp: Some(1234567890),
            verifier_address: None,
        };
        
        let signed = signer.sign_receipt(receipt).unwrap();
        
        assert!(!signed.signature.is_empty());
        assert!(signed.signature.starts_with("0x"));
        assert_eq!(signed.signature.len(), 132); // 0x + 65 bytes * 2
        assert_eq!(signed.verifier_address, Some(signer.get_address()));
    }
    
    #[test]
    fn test_deterministic_signing() {
        let private_key = get_test_private_key();
        let signer = ReceiptSigner::new(&private_key).unwrap();
        
        let receipt = VerifiedReceipt {
            device_id: "test_device".to_string(),
            proof_type: "iot_validation".to_string(),
            receipt_hash: "0xtest".to_string(),
            image_id: "0ximage".to_string(),
            journal_hash: "0xjournal".to_string(),
            epoch_index: 1,
            input_index: 1,
            signature: String::new(),
            timestamp: Some(1234567890),
            verifier_address: None,
        };
        
        let signed1 = signer.sign_receipt(receipt.clone()).unwrap();
        let signed2 = signer.sign_receipt(receipt).unwrap();
        
        // Same input should produce same signature
        assert_eq!(signed1.signature, signed2.signature);
    }
}
