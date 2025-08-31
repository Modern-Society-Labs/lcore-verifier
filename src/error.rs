//! Error types for the verifier service

use thiserror::Error;

#[derive(Error, Debug)]
pub enum VerifierError {
    #[error("Configuration error: {0}")]
    Config(String),
    
    #[error("GraphQL query error: {0}")]
    GraphQL(String),
    
    #[error("Proof verification failed: {0}")]
    ProofVerification(String),
    
    #[error("Invalid image ID: expected {expected}, got {actual}")]
    InvalidImageId { expected: String, actual: String },
    
    #[error("Receipt signing error: {0}")]
    Signing(String),
    
    #[error("InputBox submission error: {0}")]
    InputBox(String),
    
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),
    
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    
    #[error("Hex encoding error: {0}")]
    Hex(#[from] hex::FromHexError),
    
    #[error("Receipt too large: {size} bytes exceeds maximum {max} bytes")]
    ReceiptTooLarge { size: usize, max: usize },
}

pub type Result<T> = std::result::Result<T, VerifierError>;
