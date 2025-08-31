//! GraphQL client for querying Cartesi node

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::time::{timeout, sleep};
use tracing::{info, warn, error, debug};
use crate::types::ProofRequest;
use crate::error::VerifierError;

#[derive(Serialize)]
struct GraphQLRequest {
    query: String,
    variables: Option<serde_json::Value>,
}

#[derive(Deserialize)]
struct GraphQLResponse<T> {
    data: Option<T>,
    errors: Option<Vec<GraphQLError>>,
}

#[derive(Deserialize)]
struct GraphQLError {
    message: String,
}

#[derive(Deserialize)]
struct NoticesData {
    notices: NoticesConnection,
}

#[derive(Deserialize)]
struct NoticesConnection {
    edges: Vec<NoticeEdge>,
}

#[derive(Deserialize)]
struct NoticeEdge {
    node: NoticeNode,
}

#[derive(Deserialize)]
struct NoticeNode {
    index: String,
    input: InputNode,
    payload: String,
}

#[derive(Deserialize)]
struct InputNode {
    index: String,
}

pub struct GraphQLClient {
    endpoint: String,
    client: reqwest::Client,
    max_retries: u32,
    retry_delay: Duration,
    request_timeout: Duration,
}

impl GraphQLClient {
    pub fn new(endpoint: &str) -> Result<Self> {
        Ok(Self {
            endpoint: endpoint.to_string(),
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(30))
                .build()?,
            max_retries: 3,
            retry_delay: Duration::from_secs(2),
            request_timeout: Duration::from_secs(30),
        })
    }
    
    /// Execute GraphQL request with retry logic
    async fn execute_with_retry<T>(&self, request: &GraphQLRequest) -> Result<T>
    where
        T: for<'de> serde::Deserialize<'de>,
    {
        let mut last_error = None;
        
        for attempt in 1..=self.max_retries {
            debug!("GraphQL attempt {}/{} to {}", attempt, self.max_retries, self.endpoint);
            
            match timeout(self.request_timeout, self.execute_request::<T>(request)).await {
                Ok(Ok(result)) => {
                    if attempt > 1 {
                        info!("GraphQL request succeeded on attempt {}", attempt);
                    }
                    return Ok(result);
                }
                Ok(Err(e)) => {
                    warn!("GraphQL request failed on attempt {}: {}", attempt, e);
                    last_error = Some(e);
                }
                Err(_) => {
                    warn!("GraphQL request timed out on attempt {}", attempt);
                    last_error = Some(VerifierError::GraphQL("Request timeout".to_string()).into());
                }
            }
            
            if attempt < self.max_retries {
                let delay = self.retry_delay * attempt;
                debug!("Waiting {:?} before retry...", delay);
                sleep(delay).await;
            }
        }
        
        error!("GraphQL request failed after {} attempts", self.max_retries);
        Err(last_error.unwrap_or_else(|| VerifierError::GraphQL("All retry attempts failed".to_string()).into()))
    }
    
    /// Execute single GraphQL request
    async fn execute_request<T>(&self, request: &GraphQLRequest) -> Result<T>
    where
        T: for<'de> serde::Deserialize<'de>,
    {
        let response = self.client
            .post(&self.endpoint)
            .json(request)
            .send()
            .await?;
        
        // Check for HTTP errors (502, 503, 504)
        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            
            if status.as_u16() >= 502 && status.as_u16() <= 504 {
                return Err(VerifierError::GraphQL(format!("Server busy ({}): {}", status, error_text)).into());
            } else {
                return Err(VerifierError::GraphQL(format!("HTTP error ({}): {}", status, error_text)).into());
            }
        }
        
        let graphql_response: GraphQLResponse<T> = response.json().await?;
        
        if let Some(errors) = graphql_response.errors {
            return Err(VerifierError::GraphQL(
                errors.iter()
                    .map(|e| &e.message)
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", ")
            ).into());
        }
        
        graphql_response.data
            .ok_or_else(|| VerifierError::GraphQL("No data in response".to_string()).into())
    }
    
    /// Query for proof request notices
    pub async fn query_proof_requests(&self) -> Result<Vec<ProofRequest>> {
        // Query for notices with risc0_proof_request payload
        let query = r#"
            query GetProofRequests {
                notices(first: 100) {
                    edges {
                        node {
                            index
                            input {
                                index
                            }
                            payload
                        }
                    }
                }
            }
        "#;
        
        let request = GraphQLRequest {
            query: query.to_string(),
            variables: None,
        };
        
        let data: NoticesData = self.execute_with_retry(&request).await?;
        
        // Parse notices and filter for proof requests
        let mut requests = Vec::new();
        
        for edge in data.notices.edges {
            let payload_hex = edge.node.payload.trim_start_matches("0x");
            
            // Decode hex payload
            let payload_bytes = hex::decode(payload_hex)?;
            let payload_str = String::from_utf8(payload_bytes)
                .map_err(|e| VerifierError::GraphQL(format!("Invalid UTF-8 in payload: {}", e)))?;
            
            // Try to parse as JSON
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&payload_str) {
                // Check if this is a proof request
                if json.get("type").and_then(|v| v.as_str()) == Some("risc0_proof_request") {
                    if let Ok(request) = serde_json::from_value::<ProofRequest>(json["data"].clone()) {
                        requests.push(request);
                    }
                }
            }
        }
        
        Ok(requests)
    }
    
    /// Check if a receipt has already been processed
    pub async fn check_receipt_processed(&self, receipt_hash: &str) -> Result<bool> {
        // Query for inputs containing this receipt hash
        let query = r#"
            query CheckReceipt($payload: String!) {
                inputs(where: { payload: { contains: $payload } }) {
                    edges {
                        node {
                            index
                        }
                    }
                }
            }
        "#;
        
        let variables = serde_json::json!({
            "payload": receipt_hash
        });
        
        let request = GraphQLRequest {
            query: query.to_string(),
            variables: Some(variables),
        };
        
        let data: serde_json::Value = self.execute_with_retry(&request).await?;
        
        if let Some(data) = Some(data) {
            if let Some(edges) = data["inputs"]["edges"].as_array() {
                return Ok(!edges.is_empty());
            }
        }
        
        Ok(false)
    }
}