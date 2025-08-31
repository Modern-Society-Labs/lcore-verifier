//! L{CORE} RISC Zero Proof Verifier Side-car Service
//! 
//! This service runs alongside the Cartesi node to handle RISC Zero proof verification.
//! It polls for proof requests, verifies proofs, and submits signed receipts.

mod config;
mod error;
mod graphql;
mod proof_verifier;
mod receipt_signer;
mod inputbox_client;
mod types;

use anyhow::Result;
use clap::Parser;
use tracing::{info, warn, error};
use std::time::Duration;
use tokio::time::interval;
use warp::Filter;

use crate::config::Config;
use crate::graphql::GraphQLClient;
use crate::proof_verifier::ProofVerifier;
use crate::receipt_signer::ReceiptSigner;
use crate::inputbox_client::InputBoxClient;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to configuration file
    #[arg(short, long, default_value = "verifier.toml")]
    config: String,
    
    /// Enable verbose logging
    #[arg(short, long)]
    verbose: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Parse command line arguments
    let args = Args::parse();
    
    // Initialize logging
    let filter = if args.verbose { "debug" } else { "info" };
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .init();
    
    info!("Starting L{{CORE}} RISC Zero Proof Verifier");
    
    // Load configuration (prioritize environment variables)
    let config = Config::load(&args.config).unwrap_or_else(|e| {
        warn!("Failed to load config file {}: {}. Using environment variables.", args.config, e);
        Config::from_env().expect("Failed to load configuration from environment variables")
    });
    info!("Configuration loaded successfully");
    
    // Initialize components
    let graphql_client = GraphQLClient::new(&config.graphql_endpoint)?;
    let proof_verifier = ProofVerifier::new(config.allowed_image_ids.clone());
    let receipt_signer = ReceiptSigner::new(&config.verifier_private_key)?;
    let inputbox_client = InputBoxClient::new(&config.inputbox_endpoint, &config.dapp_address)?;
    
    info!("All components initialized successfully");
    info!("Polling interval: {} seconds", config.poll_interval_secs);
    
    // Start health check server
    let health_check = warp::path("health")
        .map(|| warp::reply::with_status("OK", warp::http::StatusCode::OK));
    
    let health_server = warp::serve(health_check)
        .run(([0, 0, 0, 0], 8080));
    
    info!("Health check server started on port 8080");
    
    // Main polling loop
    let mut poll_interval = interval(Duration::from_secs(config.poll_interval_secs));
    
    // Run health server and polling loop concurrently
    tokio::select! {
        _ = health_server => {
            error!("Health server stopped unexpectedly");
        }
        _ = async {
            loop {
                poll_interval.tick().await;
                
                match process_proof_requests(
                    &graphql_client,
                    &proof_verifier,
                    &receipt_signer,
                    &inputbox_client,
                    &config,
                ).await {
                    Ok(count) => {
                        if count > 0 {
                            info!("Processed {} proof requests", count);
                        }
                    }
                    Err(e) => {
                        error!("Error processing proof requests: {}", e);
                    }
                }
            }
        } => {
            error!("Polling loop stopped unexpectedly");
        }
    }
    
    Ok(())
}

/// Process all pending proof requests
async fn process_proof_requests(
    graphql: &GraphQLClient,
    verifier: &ProofVerifier,
    signer: &ReceiptSigner,
    inputbox: &InputBoxClient,
    config: &Config,
) -> Result<usize> {
    // Query for proof request notices
    let requests = graphql.query_proof_requests().await?;
    
    if requests.is_empty() {
        return Ok(0);
    }
    
    info!("Found {} proof requests to process", requests.len());
    
    let mut processed = 0;
    
    for request in requests {
        match process_single_request(request, verifier, signer, inputbox, config).await {
            Ok(()) => processed += 1,
            Err(e) => {
                warn!("Failed to process request: {}", e);
                // Continue processing other requests
            }
        }
    }
    
    Ok(processed)
}

/// Process a single proof request
async fn process_single_request(
    request: types::ProofRequest,
    verifier: &ProofVerifier,
    signer: &ReceiptSigner,
    inputbox: &InputBoxClient,
    config: &Config,
) -> Result<()> {
    info!("Processing proof request from device: {}", request.device_id);
    
    // Fetch the RISC Zero receipt
    let receipt_bytes = fetch_receipt(&request.receipt_url, config).await?;
    
    // Verify the proof
    let receipt = verifier.verify_proof(&receipt_bytes, &request.proof_type)?;
    
    // Extract journal data
    let journal_hash = receipt.journal_hash();
    
    // Create verified receipt
    let verified_receipt = types::VerifiedReceipt {
        device_id: request.device_id.clone(),
        proof_type: request.proof_type.clone(),
        receipt_hash: hex::encode(receipt.receipt_hash()),
        image_id: hex::encode(&request.expected_image_id),
        journal_hash: hex::encode(journal_hash),
        epoch_index: request.epoch_index,
        input_index: request.input_index,
        signature: String::new(), // Will be filled by signer
        timestamp: Some(chrono::Utc::now().timestamp() as u64),
        verifier_address: Some(signer.get_address()),
    };
    
    // Sign the receipt
    let signed_receipt = signer.sign_receipt(verified_receipt)?;
    
    // Submit to InputBox
    inputbox.submit_verified_receipt(&signed_receipt).await?;
    
    info!("Successfully submitted verified receipt for device: {}", request.device_id);
    
    Ok(())
}

/// Fetch receipt from URL (supports IPFS, HTTP, S3)
async fn fetch_receipt(url: &str, config: &Config) -> Result<Vec<u8>> {
    if url.starts_with("ipfs://") {
        // Convert to HTTP gateway URL
        let hash = url.trim_start_matches("ipfs://");
        let gateway_url = format!("{}/ipfs/{}", config.ipfs_gateway, hash);
        
        info!("Fetching receipt from IPFS: {}", gateway_url);
        let response = reqwest::get(&gateway_url).await?;
        let bytes = response.bytes().await?;
        Ok(bytes.to_vec())
    } else if url.starts_with("http://") || url.starts_with("https://") {
        info!("Fetching receipt from HTTP: {}", url);
        let response = reqwest::get(url).await?;
        let bytes = response.bytes().await?;
        Ok(bytes.to_vec())
    } else {
        Err(anyhow::anyhow!("Unsupported receipt URL scheme: {}", url))
    }
}