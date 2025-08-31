//! Client for submitting to Cartesi InputBox

use anyhow::Result;
use reqwest::Client;
use crate::types::{VerifiedReceipt, InputBoxPayload};
use crate::error::VerifierError;
use tracing::{info, debug};

pub struct InputBoxClient {
    endpoint: String,
    dapp_address: String,
    client: Client,
}

impl InputBoxClient {
    pub fn new(endpoint: &str, dapp_address: &str) -> Result<Self> {
        // Normalize DApp address
        let dapp_address = if dapp_address.starts_with("0x") {
            dapp_address.to_string()
        } else {
            format!("0x{}", dapp_address)
        };
        
        Ok(Self {
            endpoint: endpoint.to_string(),
            dapp_address,
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()?,
        })
    }
    
    /// Submit a verified receipt to the InputBox
    pub async fn submit_verified_receipt(&self, receipt: &VerifiedReceipt) -> Result<()> {
        // Create command wrapper
        let command = serde_json::json!({
            "command": "submit_verified_receipt",
            "data": receipt
        });
        
        // Encode as hex
        let payload_json = serde_json::to_string(&command)?;
        let payload_hex = format!("0x{}", hex::encode(payload_json));
        
        // Create InputBox payload
        let input_payload = InputBoxPayload {
            address: self.dapp_address.clone(),
            payload: payload_hex,
        };
        
        debug!("Submitting verified receipt to InputBox: {}", self.endpoint);
        debug!("DApp address: {}", self.dapp_address);
        debug!("Receipt hash: {}", receipt.receipt_hash);
        
        // Submit to InputBox
        let response = self.client
            .post(&self.endpoint)
            .json(&input_payload)
            .send()
            .await
            .map_err(|e| VerifierError::InputBox(format!("Failed to send request: {}", e)))?;
        
        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(VerifierError::InputBox(
                format!("InputBox returned error {}: {}", status, error_text)
            ).into());
        }
        
        // Parse response to get input index
        let response_data: serde_json::Value = response.json().await?;
        
        if let Some(index) = response_data.get("index") {
            info!("Verified receipt submitted successfully with index: {}", index);
        } else {
            info!("Verified receipt submitted successfully");
        }
        
        Ok(())
    }
    
    /// Health check for InputBox
    pub async fn health_check(&self) -> Result<bool> {
        let health_url = format!("{}/health", self.endpoint.trim_end_matches("/input"));
        
        match self.client.get(&health_url).send().await {
            Ok(response) => Ok(response.status().is_success()),
            Err(_) => Ok(false),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_dapp_address_normalization() {
        // Test with 0x prefix
        let client1 = InputBoxClient::new("http://localhost:8080/input", "0x1234567890abcdef1234567890abcdef12345678")
            .unwrap();
        assert_eq!(client1.dapp_address, "0x1234567890abcdef1234567890abcdef12345678");
        
        // Test without 0x prefix
        let client2 = InputBoxClient::new("http://localhost:8080/input", "1234567890abcdef1234567890abcdef12345678")
            .unwrap();
        assert_eq!(client2.dapp_address, "0x1234567890abcdef1234567890abcdef12345678");
    }
}
