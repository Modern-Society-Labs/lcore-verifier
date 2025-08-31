//! Configuration management for the verifier service

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::env;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// GraphQL endpoint for querying proof requests
    pub graphql_endpoint: String,
    
    /// InputBox HTTP endpoint for submitting receipts
    pub inputbox_endpoint: String,
    
    /// DApp address for InputBox submissions
    pub dapp_address: String,
    
    /// Private key for signing verified receipts
    pub verifier_private_key: String,
    
    /// Allowed RISC Zero image IDs
    pub allowed_image_ids: Vec<String>,
    
    /// Polling interval in seconds
    pub poll_interval_secs: u64,
    
    /// IPFS gateway for fetching receipts
    pub ipfs_gateway: String,
    
    /// Maximum receipt size in bytes
    pub max_receipt_size: usize,
    
    /// Request timeout in seconds
    pub request_timeout_secs: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            graphql_endpoint: "http://localhost:8000/graphql".to_string(),
            inputbox_endpoint: "http://localhost:8080/input".to_string(),
            dapp_address: "0x0000000000000000000000000000000000000000".to_string(),
            verifier_private_key: String::new(),
            allowed_image_ids: vec![],
            poll_interval_secs: 10,
            ipfs_gateway: "https://ipfs.io".to_string(),
            max_receipt_size: 10 * 1024 * 1024, // 10 MB
            request_timeout_secs: 30,
        }
    }
}

impl Config {
    /// Load configuration from file and environment
    pub fn load(path: &str) -> Result<Self> {
        // Start with defaults
        let mut config = if std::path::Path::new(path).exists() {
            // Load from file if it exists
            let contents = fs::read_to_string(path)?;
            toml::from_str(&contents)?
        } else {
            // Use defaults
            Config::default()
        };
        
        // Override with environment variables
        if let Ok(endpoint) = env::var("GRAPHQL_ENDPOINT") {
            config.graphql_endpoint = endpoint;
        }
        
        if let Ok(endpoint) = env::var("INPUTBOX_ENDPOINT") {
            config.inputbox_endpoint = endpoint;
        }
        
        if let Ok(address) = env::var("DAPP_ADDRESS") {
            config.dapp_address = address;
        }
        
        if let Ok(key) = env::var("VERIFIER_PRIVATE_KEY") {
            config.verifier_private_key = key;
        }
        
        if let Ok(ids) = env::var("ALLOWED_IMAGE_IDS") {
            config.allowed_image_ids = ids.split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
        }
        
        if let Ok(interval) = env::var("POLL_INTERVAL_SECS") {
            if let Ok(secs) = interval.parse() {
                config.poll_interval_secs = secs;
            }
        }
        
        if let Ok(gateway) = env::var("IPFS_GATEWAY") {
            config.ipfs_gateway = gateway;
        }
        
        // Validate configuration
        config.validate()?;
        
        Ok(config)
    }
    
    /// Load configuration from environment variables only
    pub fn from_env() -> Result<Self> {
        let mut config = Config::default();
        
        // Override with environment variables
        if let Ok(endpoint) = env::var("GRAPHQL_ENDPOINT") {
            config.graphql_endpoint = endpoint;
        }
        
        if let Ok(endpoint) = env::var("INPUTBOX_ENDPOINT") {
            config.inputbox_endpoint = endpoint;
        }
        
        if let Ok(address) = env::var("DAPP_ADDRESS") {
            config.dapp_address = address;
        }
        
        if let Ok(key) = env::var("VERIFIER_PRIVATE_KEY") {
            config.verifier_private_key = key;
        }
        
        if let Ok(ids) = env::var("ALLOWED_IMAGE_IDS") {
            config.allowed_image_ids = ids.split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
        }
        
        if let Ok(interval) = env::var("POLL_INTERVAL_SECS") {
            if let Ok(secs) = interval.parse() {
                config.poll_interval_secs = secs;
            }
        }
        
        if let Ok(gateway) = env::var("IPFS_GATEWAY") {
            config.ipfs_gateway = gateway;
        }
        
        // Validate configuration
        config.validate()?;
        
        Ok(config)
    }
    
    /// Validate configuration values
    fn validate(&self) -> Result<()> {
        if self.verifier_private_key.is_empty() {
            return Err(anyhow::anyhow!("Verifier private key is required"));
        }
        
        if self.allowed_image_ids.is_empty() {
            return Err(anyhow::anyhow!("At least one allowed image ID is required"));
        }
        
        Ok(())
    }
}