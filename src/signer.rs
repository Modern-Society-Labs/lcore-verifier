// ECDSA Signing for Verified Receipts

use anyhow::{anyhow, Result};
use ethers::prelude::*;
use k256::ecdsa::{SigningKey, Signature, signature::Signer as K256Signer};
use sha3::{Digest, Keccak256};
use hex;

pub struct VerifierSigner {
    signing_key: SigningKey,
    address: Address,
}

impl VerifierSigner {
    pub fn new(private_key_hex: &str) -> Result<Self> {
        let private_key = private_key_hex
            .strip_prefix("0x")
            .unwrap_or(private_key_hex);
            
        let key_bytes = hex::decode(private_key)
            .map_err(|e| anyhow!("Invalid private key hex: {}", e))?;
            
        let signing_key = SigningKey::from_slice(&key_bytes)
            .map_err(|e| anyhow!("Invalid private key: {}", e))?;
            
        // Derive address from public key
        let public_key = signing_key.verifying_key();
        let public_key_bytes = public_key.to_encoded_point(false);
        let public_key_bytes = &public_key_bytes.as_bytes()[1..]; // Remove 0x04 prefix
        
        let mut hasher = Keccak256::new();
        hasher.update(public_key_bytes);
        let hash = hasher.finalize();
        let address_bytes = &hash[12..];
        let address = Address::from_slice(address_bytes);
        
        Ok(Self {
            signing_key,
            address,
        })
    }
    
    pub fn address(&self) -> String {
        format!("0x{}", hex::encode(self.address.as_bytes()))
    }
    
    pub async fn sign_receipt(&self, receipt: &super::VerifiedReceipt) -> Result<String> {
        // Compute the hash to sign
        let message_hash = compute_receipt_hash(receipt);
        
        // Add Ethereum prefix
        let mut eth_message = Vec::new();
        eth_message.extend_from_slice(b"\x19Ethereum Signed Message:\n32");
        eth_message.extend_from_slice(&message_hash);
        
        let eth_hash = Keccak256::digest(&eth_message);
        
        // Sign the hash
        let (signature, recovery_id) = self.signing_key
            .sign_prehash_recoverable(&eth_hash)
            .map_err(|e| anyhow!("Failed to sign: {}", e))?;
        
        // Convert to Ethereum signature format (r, s, v)
        let mut eth_sig = vec![0u8; 65];
        let sig_bytes = signature.to_bytes();
        eth_sig[..32].copy_from_slice(&sig_bytes[..32]); // r
        eth_sig[32..64].copy_from_slice(&sig_bytes[32..]); // s
        eth_sig[64] = 27 + recovery_id.to_byte(); // v = 27 or 28
        
        Ok(format!("0x{}", hex::encode(eth_sig)))
    }
}

fn compute_receipt_hash(receipt: &super::VerifiedReceipt) -> [u8; 32] {
    let mut hasher = Keccak256::new();
    
    // Hash all fields in deterministic order (excluding signature)
    hasher.update(receipt.device_id.as_bytes());
    hasher.update(receipt.proof_type.as_bytes());
    hasher.update(receipt.receipt_hash.as_bytes());
    hasher.update(receipt.image_id.as_bytes());
    hasher.update(receipt.journal_hash.as_bytes());
    hasher.update(&receipt.epoch_index.to_le_bytes());
    hasher.update(&receipt.input_index.to_le_bytes());
    
    hasher.finalize().into()
}
