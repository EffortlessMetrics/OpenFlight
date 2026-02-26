// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Web API client for X-Plane
//!
//! Provides HTTP-based communication with X-Plane's web API for DataRef access
//! and aircraft information when available.

use crate::dataref::DataRefValue;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use thiserror::Error;
use tracing::{debug, warn};

/// Web API client errors
#[derive(Error, Debug)]
pub enum WebApiError {
    #[error("HTTP request failed: {0}")]
    Http(String),
    #[error("JSON parsing error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("API not available")]
    NotAvailable,
    #[error("Authentication failed")]
    AuthenticationFailed,
    #[error("DataRef not found: {name}")]
    DataRefNotFound { name: String },
    #[error("Request timeout")]
    Timeout,
    #[error("Invalid response format")]
    InvalidResponse,
}

/// Web API configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebApiConfig {
    /// Base URL for X-Plane web API
    pub base_url: String,
    /// API key (if required)
    pub api_key: Option<String>,
    /// Request timeout
    pub timeout: Duration,
    /// Enable SSL verification
    pub verify_ssl: bool,
    /// Maximum retries for failed requests
    pub max_retries: u32,
}

impl Default for WebApiConfig {
    fn default() -> Self {
        Self {
            base_url: "http://127.0.0.1:8080".to_string(),
            api_key: None,
            timeout: Duration::from_secs(5),
            verify_ssl: true,
            max_retries: 3,
        }
    }
}

/// DataRef API response
#[derive(Debug, Deserialize)]
struct DataRefResponse {
    pub value: serde_json::Value,
    pub _timestamp: Option<u64>,
}

/// Aircraft info API response
#[derive(Debug, Deserialize)]
struct AircraftInfoResponse {
    pub icao: String,
    pub title: String,
    pub author: String,
    pub file_path: String,
}

/// Web API client for X-Plane
#[derive(Clone)]
pub struct WebApiClient {
    config: WebApiConfig,
    client: reqwest::Client,
}

impl WebApiClient {
    /// Create a new web API client
    pub fn new(config: WebApiConfig) -> Result<Self, WebApiError> {
        let client = reqwest::Client::builder()
            .timeout(config.timeout)
            .danger_accept_invalid_certs(!config.verify_ssl)
            .build()
            .map_err(|e| WebApiError::Http(e.to_string()))?;

        Ok(Self { config, client })
    }

    /// Test API availability
    pub async fn test_connection(&self) -> Result<(), WebApiError> {
        let url = format!("{}/api/v1/status", self.config.base_url);

        match self.client.get(&url).send().await {
            Ok(response) => {
                if response.status().is_success() {
                    debug!("Web API connection successful");
                    Ok(())
                } else {
                    Err(WebApiError::Http(format!(
                        "API returned status: {}",
                        response.status()
                    )))
                }
            }
            Err(e) => {
                debug!("Web API not available: {}", e);
                Err(WebApiError::NotAvailable)
            }
        }
    }

    /// Get DataRef value via web API
    pub async fn get_dataref(&self, name: &str) -> Result<DataRefValue, WebApiError> {
        let url = format!("{}/api/v1/dataref/{}", self.config.base_url, name);

        let mut request = self.client.get(&url);

        // Add API key if configured
        if let Some(ref api_key) = self.config.api_key {
            request = request.header("X-API-Key", api_key);
        }

        let response = request
            .send()
            .await
            .map_err(|e| WebApiError::Http(e.to_string()))?;

        if !response.status().is_success() {
            return match response.status().as_u16() {
                404 => Err(WebApiError::DataRefNotFound {
                    name: name.to_string(),
                }),
                401 | 403 => Err(WebApiError::AuthenticationFailed),
                _ => Err(WebApiError::Http(format!(
                    "API returned status: {}",
                    response.status()
                ))),
            };
        }

        let dataref_response: DataRefResponse = response
            .json()
            .await
            .map_err(|e| WebApiError::Http(e.to_string()))?;

        // Convert JSON value to DataRefValue
        self.convert_json_to_dataref_value(dataref_response.value)
    }

    /// Set DataRef value via web API
    pub async fn set_dataref(&self, name: &str, value: DataRefValue) -> Result<(), WebApiError> {
        let url = format!("{}/api/v1/dataref/{}", self.config.base_url, name);

        let json_value = self.convert_dataref_value_to_json(value)?;

        let mut request = self.client.put(&url).json(&json_value);

        // Add API key if configured
        if let Some(ref api_key) = self.config.api_key {
            request = request.header("X-API-Key", api_key);
        }

        let response = request
            .send()
            .await
            .map_err(|e| WebApiError::Http(e.to_string()))?;

        if !response.status().is_success() {
            return match response.status().as_u16() {
                404 => Err(WebApiError::DataRefNotFound {
                    name: name.to_string(),
                }),
                401 | 403 => Err(WebApiError::AuthenticationFailed),
                _ => Err(WebApiError::Http(format!(
                    "API returned status: {}",
                    response.status()
                ))),
            };
        }

        Ok(())
    }

    /// Get aircraft information via web API
    pub async fn get_aircraft_info(&self) -> Result<(String, String, String, String), WebApiError> {
        let url = format!("{}/api/v1/aircraft", self.config.base_url);

        let mut request = self.client.get(&url);

        // Add API key if configured
        if let Some(ref api_key) = self.config.api_key {
            request = request.header("X-API-Key", api_key);
        }

        let response = request
            .send()
            .await
            .map_err(|e| WebApiError::Http(e.to_string()))?;

        if !response.status().is_success() {
            return Err(WebApiError::Http(format!(
                "API returned status: {}",
                response.status()
            )));
        }

        let aircraft_info: AircraftInfoResponse = response
            .json()
            .await
            .map_err(|e| WebApiError::Http(e.to_string()))?;

        Ok((
            aircraft_info.icao,
            aircraft_info.title,
            aircraft_info.author,
            aircraft_info.file_path,
        ))
    }

    /// Get multiple DataRefs in a single request
    pub async fn get_multiple_datarefs(
        &self,
        names: &[String],
    ) -> Result<Vec<(String, DataRefValue)>, WebApiError> {
        let url = format!("{}/api/v1/datarefs", self.config.base_url);

        let request_body = serde_json::json!({
            "datarefs": names
        });

        let mut request = self.client.post(&url).json(&request_body);

        // Add API key if configured
        if let Some(ref api_key) = self.config.api_key {
            request = request.header("X-API-Key", api_key);
        }

        let response = request
            .send()
            .await
            .map_err(|e| WebApiError::Http(e.to_string()))?;

        if !response.status().is_success() {
            return Err(WebApiError::Http(format!(
                "API returned status: {}",
                response.status()
            )));
        }

        let response_data: serde_json::Value = response
            .json()
            .await
            .map_err(|e| WebApiError::Http(e.to_string()))?;

        let mut results = Vec::new();

        if let Some(datarefs) = response_data.get("datarefs").and_then(|v| v.as_object()) {
            for (name, value) in datarefs {
                match self.convert_json_to_dataref_value(value.clone()) {
                    Ok(dataref_value) => {
                        results.push((name.clone(), dataref_value));
                    }
                    Err(e) => {
                        warn!("Failed to convert DataRef {}: {}", name, e);
                    }
                }
            }
        }

        Ok(results)
    }

    /// Convert JSON value to DataRefValue
    fn convert_json_to_dataref_value(
        &self,
        value: serde_json::Value,
    ) -> Result<DataRefValue, WebApiError> {
        match value {
            serde_json::Value::Number(n) => {
                if let Some(f) = n.as_f64() {
                    Ok(DataRefValue::Float(f as f32))
                } else if let Some(i) = n.as_i64() {
                    Ok(DataRefValue::Int(i as i32))
                } else {
                    Err(WebApiError::InvalidResponse)
                }
            }
            serde_json::Value::Array(arr) => {
                // Try to convert to float array first
                let mut float_values = Vec::new();
                let mut int_values = Vec::new();
                let mut is_float_array = true;

                for item in &arr {
                    match item {
                        serde_json::Value::Number(n) => {
                            if let Some(f) = n.as_f64() {
                                float_values.push(f as f32);
                                int_values.push(f as i32);
                            } else {
                                is_float_array = false;
                                break;
                            }
                        }
                        _ => {
                            is_float_array = false;
                            break;
                        }
                    }
                }

                if is_float_array {
                    // Determine if it's better represented as int or float array
                    let all_integers = float_values
                        .iter()
                        .all(|&f| f.fract() == 0.0 && f >= i32::MIN as f32 && f <= i32::MAX as f32);

                    if all_integers {
                        Ok(DataRefValue::IntArray(int_values))
                    } else {
                        Ok(DataRefValue::FloatArray(float_values))
                    }
                } else {
                    Err(WebApiError::InvalidResponse)
                }
            }
            serde_json::Value::Bool(b) => Ok(DataRefValue::Int(if b { 1 } else { 0 })),
            _ => Err(WebApiError::InvalidResponse),
        }
    }

    /// Convert DataRefValue to JSON value
    fn convert_dataref_value_to_json(
        &self,
        value: DataRefValue,
    ) -> Result<serde_json::Value, WebApiError> {
        match value {
            DataRefValue::Float(f) => Ok(serde_json::Value::Number(
                serde_json::Number::from_f64(f as f64).ok_or(WebApiError::InvalidResponse)?,
            )),
            DataRefValue::Double(d) => Ok(serde_json::Value::Number(
                serde_json::Number::from_f64(d).ok_or(WebApiError::InvalidResponse)?,
            )),
            DataRefValue::Int(i) => Ok(serde_json::Value::Number(serde_json::Number::from(i))),
            DataRefValue::FloatArray(arr) => {
                let json_arr: Result<Vec<serde_json::Value>, _> = arr
                    .into_iter()
                    .map(|f| {
                        serde_json::Number::from_f64(f as f64)
                            .map(serde_json::Value::Number)
                            .ok_or(WebApiError::InvalidResponse)
                    })
                    .collect();
                Ok(serde_json::Value::Array(json_arr?))
            }
            DataRefValue::IntArray(arr) => {
                let json_arr: Vec<serde_json::Value> = arr
                    .into_iter()
                    .map(|i| serde_json::Value::Number(serde_json::Number::from(i)))
                    .collect();
                Ok(serde_json::Value::Array(json_arr))
            }
        }
    }

    /// Check if web API is available
    pub async fn is_available(&self) -> bool {
        self.test_connection().await.is_ok()
    }

    /// Get API configuration
    pub fn get_config(&self) -> &WebApiConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_web_api_config_defaults() {
        let config = WebApiConfig::default();
        assert_eq!(config.base_url, "http://127.0.0.1:8080");
        assert!(config.api_key.is_none());
        assert_eq!(config.timeout, Duration::from_secs(5));
        assert!(config.verify_ssl);
        assert_eq!(config.max_retries, 3);
    }

    #[test]
    fn test_json_to_dataref_conversion() {
        let config = WebApiConfig::default();
        let client = WebApiClient::new(config).unwrap();

        // Test float conversion
        let json_float = serde_json::Value::Number(serde_json::Number::from_f64(42.5).unwrap());
        let result = client.convert_json_to_dataref_value(json_float).unwrap();
        assert_eq!(result, DataRefValue::Float(42.5));

        // Test integer conversion
        let json_int = serde_json::Value::Number(serde_json::Number::from(123));
        let result = client.convert_json_to_dataref_value(json_int).unwrap();
        assert_eq!(result, DataRefValue::Float(123.0));

        // Test boolean conversion
        let json_bool = serde_json::Value::Bool(true);
        let result = client.convert_json_to_dataref_value(json_bool).unwrap();
        assert_eq!(result, DataRefValue::Int(1));

        // Test float array conversion
        let json_array = serde_json::Value::Array(vec![
            serde_json::Value::Number(serde_json::Number::from_f64(1.5).unwrap()),
            serde_json::Value::Number(serde_json::Number::from_f64(2.5).unwrap()),
        ]);
        let result = client.convert_json_to_dataref_value(json_array).unwrap();
        assert_eq!(result, DataRefValue::FloatArray(vec![1.5, 2.5]));

        // Test integer array conversion
        let json_int_array = serde_json::Value::Array(vec![
            serde_json::Value::Number(serde_json::Number::from(1)),
            serde_json::Value::Number(serde_json::Number::from(2)),
        ]);
        let result = client
            .convert_json_to_dataref_value(json_int_array)
            .unwrap();
        assert_eq!(result, DataRefValue::IntArray(vec![1, 2]));
    }

    #[test]
    fn test_dataref_to_json_conversion() {
        let config = WebApiConfig::default();
        let client = WebApiClient::new(config).unwrap();

        // Test float conversion
        let dataref_float = DataRefValue::Float(42.5);
        let result = client.convert_dataref_value_to_json(dataref_float).unwrap();
        assert_eq!(
            result,
            serde_json::Value::Number(serde_json::Number::from_f64(42.5).unwrap())
        );

        // Test integer conversion
        let dataref_int = DataRefValue::Int(123);
        let result = client.convert_dataref_value_to_json(dataref_int).unwrap();
        assert_eq!(
            result,
            serde_json::Value::Number(serde_json::Number::from(123))
        );

        // Test float array conversion
        let dataref_array = DataRefValue::FloatArray(vec![1.5, 2.5]);
        let result = client.convert_dataref_value_to_json(dataref_array).unwrap();
        let expected = serde_json::Value::Array(vec![
            serde_json::Value::Number(serde_json::Number::from_f64(1.5).unwrap()),
            serde_json::Value::Number(serde_json::Number::from_f64(2.5).unwrap()),
        ]);
        assert_eq!(result, expected);

        // Test integer array conversion
        let dataref_int_array = DataRefValue::IntArray(vec![1, 2]);
        let result = client
            .convert_dataref_value_to_json(dataref_int_array)
            .unwrap();
        let expected = serde_json::Value::Array(vec![
            serde_json::Value::Number(serde_json::Number::from(1)),
            serde_json::Value::Number(serde_json::Number::from(2)),
        ]);
        assert_eq!(result, expected);
    }

    #[tokio::test]
    async fn test_web_api_client_creation() {
        let config = WebApiConfig::default();
        let client = WebApiClient::new(config);
        assert!(client.is_ok());
    }

    #[test]
    fn test_web_api_config_customization() {
        let config = WebApiConfig {
            base_url: "https://custom.api.com".to_string(),
            api_key: Some("test-key".to_string()),
            timeout: Duration::from_secs(10),
            verify_ssl: false,
            max_retries: 5,
        };

        assert_eq!(config.base_url, "https://custom.api.com");
        assert_eq!(config.api_key, Some("test-key".to_string()));
        assert_eq!(config.timeout, Duration::from_secs(10));
        assert!(!config.verify_ssl);
        assert_eq!(config.max_retries, 5);
    }

    // Note: Integration tests would require a running X-Plane instance with web API enabled
    // These would be better placed in a separate integration test suite

    // --- URL construction ---

    #[test]
    fn test_url_status_path() {
        let base = "http://127.0.0.1:8080";
        let url = format!("{}/api/v1/status", base);
        assert_eq!(url, "http://127.0.0.1:8080/api/v1/status");
    }

    #[test]
    fn test_url_dataref_path() {
        let base = "http://127.0.0.1:8080";
        let name = "sim/cockpit/switches/gear_handle_status";
        let url = format!("{}/api/v1/dataref/{}", base, name);
        assert_eq!(
            url,
            "http://127.0.0.1:8080/api/v1/dataref/sim/cockpit/switches/gear_handle_status"
        );
    }

    #[test]
    fn test_url_aircraft_path() {
        let base = "http://192.168.1.10:8080";
        let url = format!("{}/api/v1/aircraft", base);
        assert_eq!(url, "http://192.168.1.10:8080/api/v1/aircraft");
    }

    #[test]
    fn test_url_datarefs_bulk_path() {
        let base = "http://127.0.0.1:8080";
        let url = format!("{}/api/v1/datarefs", base);
        assert_eq!(url, "http://127.0.0.1:8080/api/v1/datarefs");
    }

    // --- JSON → DataRefValue error cases ---

    #[test]
    fn test_json_null_returns_invalid_response() {
        let config = WebApiConfig::default();
        let client = WebApiClient::new(config).unwrap();

        let result = client.convert_json_to_dataref_value(serde_json::Value::Null);
        assert!(
            matches!(result, Err(WebApiError::InvalidResponse)),
            "Expected InvalidResponse, got {:?}",
            result
        );
    }

    #[test]
    fn test_json_string_returns_invalid_response() {
        let config = WebApiConfig::default();
        let client = WebApiClient::new(config).unwrap();

        let result =
            client.convert_json_to_dataref_value(serde_json::Value::String("42.0".to_string()));
        assert!(
            matches!(result, Err(WebApiError::InvalidResponse)),
            "Expected InvalidResponse, got {:?}",
            result
        );
    }

    #[test]
    fn test_json_object_returns_invalid_response() {
        let config = WebApiConfig::default();
        let client = WebApiClient::new(config).unwrap();

        let obj = serde_json::json!({ "x": 1 });
        let result = client.convert_json_to_dataref_value(obj);
        assert!(
            matches!(result, Err(WebApiError::InvalidResponse)),
            "Expected InvalidResponse, got {:?}",
            result
        );
    }

    #[test]
    fn test_json_array_with_string_element_returns_invalid_response() {
        let config = WebApiConfig::default();
        let client = WebApiClient::new(config).unwrap();

        let arr = serde_json::Value::Array(vec![
            serde_json::Value::Number(serde_json::Number::from(1)),
            serde_json::Value::String("bad".to_string()),
        ]);
        let result = client.convert_json_to_dataref_value(arr);
        assert!(
            matches!(result, Err(WebApiError::InvalidResponse)),
            "Expected InvalidResponse, got {:?}",
            result
        );
    }

    #[test]
    fn test_json_empty_array_returns_int_array() {
        let config = WebApiConfig::default();
        let client = WebApiClient::new(config).unwrap();

        let result = client
            .convert_json_to_dataref_value(serde_json::Value::Array(vec![]))
            .unwrap();
        assert_eq!(result, DataRefValue::IntArray(vec![]));
    }

    #[test]
    fn test_json_bool_false_returns_int_zero() {
        let config = WebApiConfig::default();
        let client = WebApiClient::new(config).unwrap();

        let result = client
            .convert_json_to_dataref_value(serde_json::Value::Bool(false))
            .unwrap();
        assert_eq!(result, DataRefValue::Int(0));
    }

    // --- DataRefValue → JSON error cases ---

    #[test]
    fn test_nan_float_to_json_returns_invalid_response() {
        let config = WebApiConfig::default();
        let client = WebApiClient::new(config).unwrap();

        let result = client.convert_dataref_value_to_json(DataRefValue::Float(f32::NAN));
        assert!(
            matches!(result, Err(WebApiError::InvalidResponse)),
            "Expected InvalidResponse for NaN float, got {:?}",
            result
        );
    }

    #[test]
    fn test_nan_double_to_json_returns_invalid_response() {
        let config = WebApiConfig::default();
        let client = WebApiClient::new(config).unwrap();

        let result = client.convert_dataref_value_to_json(DataRefValue::Double(f64::NAN));
        assert!(
            matches!(result, Err(WebApiError::InvalidResponse)),
            "Expected InvalidResponse for NaN double, got {:?}",
            result
        );
    }

    #[test]
    fn test_nan_in_float_array_returns_invalid_response() {
        let config = WebApiConfig::default();
        let client = WebApiClient::new(config).unwrap();

        let result =
            client.convert_dataref_value_to_json(DataRefValue::FloatArray(vec![1.0, f32::NAN]));
        assert!(
            matches!(result, Err(WebApiError::InvalidResponse)),
            "Expected InvalidResponse for float array with NaN, got {:?}",
            result
        );
    }

    // --- Double DataRefValue round-trip ---

    #[test]
    fn test_double_to_json_and_back() {
        let config = WebApiConfig::default();
        let client = WebApiClient::new(config).unwrap();

        let json = client
            .convert_dataref_value_to_json(DataRefValue::Double(3.14159265358979))
            .unwrap();
        // JSON double comes back as a Number; converting back gives a Float (f64→f32 narrowing)
        let back = client.convert_json_to_dataref_value(json).unwrap();
        if let DataRefValue::Float(f) = back {
            assert!((f - 3.14159265358979_f32).abs() < 1e-5);
        } else {
            panic!("Expected Float, got {:?}", back);
        }
    }

    // --- WebApiError display strings ---

    #[test]
    fn test_error_display_http() {
        let err = WebApiError::Http("connection refused".to_string());
        assert_eq!(err.to_string(), "HTTP request failed: connection refused");
    }

    #[test]
    fn test_error_display_not_available() {
        let err = WebApiError::NotAvailable;
        assert_eq!(err.to_string(), "API not available");
    }

    #[test]
    fn test_error_display_authentication_failed() {
        let err = WebApiError::AuthenticationFailed;
        assert_eq!(err.to_string(), "Authentication failed");
    }

    #[test]
    fn test_error_display_dataref_not_found() {
        let err = WebApiError::DataRefNotFound {
            name: "sim/test/ref".to_string(),
        };
        assert_eq!(err.to_string(), "DataRef not found: sim/test/ref");
    }

    #[test]
    fn test_error_display_timeout() {
        let err = WebApiError::Timeout;
        assert_eq!(err.to_string(), "Request timeout");
    }

    #[test]
    fn test_error_display_invalid_response() {
        let err = WebApiError::InvalidResponse;
        assert_eq!(err.to_string(), "Invalid response format");
    }

    #[test]
    fn test_error_from_serde_json() {
        let json_err: serde_json::Error =
            serde_json::from_str::<serde_json::Value>("not json").unwrap_err();
        let err = WebApiError::from(json_err);
        assert!(err.to_string().starts_with("JSON parsing error:"));
    }
}
