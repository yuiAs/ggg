//! Circuit breaker pattern for HTTP requests
//!
//! Prevents repeated failed requests to unavailable servers by tracking
//! failures per domain and temporarily blocking requests.

use std::collections::HashMap;
use std::sync::RwLock;
use std::time::{Duration, Instant};

/// Circuit breaker state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
    /// Normal operation, requests allowed
    Closed,
    /// Requests blocked due to repeated failures
    Open,
    /// Testing if service recovered (single request allowed)
    HalfOpen,
}

/// Per-domain circuit state
#[derive(Debug)]
struct DomainCircuit {
    state: CircuitState,
    /// Consecutive failure count
    failures: u32,
    /// Time when circuit was opened (for cooldown)
    opened_at: Option<Instant>,
    /// Last successful request time
    last_success: Option<Instant>,
}

impl Default for DomainCircuit {
    fn default() -> Self {
        Self {
            state: CircuitState::Closed,
            failures: 0,
            opened_at: None,
            last_success: None,
        }
    }
}

/// Configuration for circuit breaker behavior
#[derive(Debug, Clone)]
pub struct CircuitBreakerConfig {
    /// Number of consecutive failures before opening circuit
    pub failure_threshold: u32,
    /// Time to wait before trying again (half-open state)
    pub cooldown_duration: Duration,
    /// Time after which a closed circuit resets failure count
    pub success_reset_duration: Duration,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            cooldown_duration: Duration::from_secs(60),
            success_reset_duration: Duration::from_secs(300),
        }
    }
}

/// Circuit breaker for managing per-domain request states
#[derive(Debug)]
pub struct CircuitBreaker {
    config: CircuitBreakerConfig,
    circuits: RwLock<HashMap<String, DomainCircuit>>,
}

impl CircuitBreaker {
    /// Create a new circuit breaker with default configuration
    pub fn new() -> Self {
        Self::with_config(CircuitBreakerConfig::default())
    }

    /// Create a new circuit breaker with custom configuration
    pub fn with_config(config: CircuitBreakerConfig) -> Self {
        Self {
            config,
            circuits: RwLock::new(HashMap::new()),
        }
    }

    /// Check if requests to a domain are allowed
    ///
    /// Returns the current circuit state for the domain
    pub fn can_request(&self, domain: &str) -> CircuitState {
        let mut circuits = self.circuits.write().unwrap();
        let circuit = circuits.entry(domain.to_string()).or_default();

        match circuit.state {
            CircuitState::Closed => CircuitState::Closed,
            CircuitState::Open => {
                // Check if cooldown has elapsed
                if let Some(opened_at) = circuit.opened_at {
                    if opened_at.elapsed() >= self.config.cooldown_duration {
                        // Transition to half-open to test
                        circuit.state = CircuitState::HalfOpen;
                        tracing::info!(
                            "Circuit for {} transitioning to half-open (testing recovery)",
                            domain
                        );
                        CircuitState::HalfOpen
                    } else {
                        CircuitState::Open
                    }
                } else {
                    CircuitState::Open
                }
            }
            CircuitState::HalfOpen => CircuitState::HalfOpen,
        }
    }

    /// Record a successful request to a domain
    pub fn record_success(&self, domain: &str) {
        let mut circuits = self.circuits.write().unwrap();
        let circuit = circuits.entry(domain.to_string()).or_default();

        circuit.failures = 0;
        circuit.last_success = Some(Instant::now());

        if circuit.state == CircuitState::HalfOpen {
            tracing::info!("Circuit for {} closed (service recovered)", domain);
        }

        circuit.state = CircuitState::Closed;
        circuit.opened_at = None;
    }

    /// Record a failed request to a domain
    ///
    /// Returns true if the circuit was just opened
    pub fn record_failure(&self, domain: &str) -> bool {
        let mut circuits = self.circuits.write().unwrap();
        let circuit = circuits.entry(domain.to_string()).or_default();

        // Reset failure count if enough time has passed since last success
        if let Some(last_success) = circuit.last_success {
            if last_success.elapsed() > self.config.success_reset_duration {
                circuit.failures = 0;
            }
        }

        circuit.failures += 1;

        // If in half-open state and failed, go back to open
        if circuit.state == CircuitState::HalfOpen {
            circuit.state = CircuitState::Open;
            circuit.opened_at = Some(Instant::now());
            tracing::warn!(
                "Circuit for {} re-opened (recovery test failed)",
                domain
            );
            return true;
        }

        // Check if we should open the circuit
        if circuit.state == CircuitState::Closed
            && circuit.failures >= self.config.failure_threshold
        {
            circuit.state = CircuitState::Open;
            circuit.opened_at = Some(Instant::now());
            tracing::warn!(
                "Circuit for {} opened after {} consecutive failures",
                domain,
                circuit.failures
            );
            return true;
        }

        false
    }

    /// Get the current state and failure count for a domain
    pub fn get_status(&self, domain: &str) -> (CircuitState, u32) {
        let circuits = self.circuits.read().unwrap();
        circuits
            .get(domain)
            .map(|c| (c.state, c.failures))
            .unwrap_or((CircuitState::Closed, 0))
    }

    /// Check if a domain's circuit is open (requests blocked)
    pub fn is_open(&self, domain: &str) -> bool {
        self.can_request(domain) == CircuitState::Open
    }

    /// Reset circuit for a domain
    pub fn reset(&self, domain: &str) {
        let mut circuits = self.circuits.write().unwrap();
        circuits.remove(domain);
        tracing::debug!("Circuit for {} reset", domain);
    }

    /// Clear all circuits
    pub fn clear_all(&self) {
        let mut circuits = self.circuits.write().unwrap();
        circuits.clear();
        tracing::debug!("All circuits cleared");
    }

    /// Get list of domains with open circuits
    pub fn get_open_circuits(&self) -> Vec<String> {
        let circuits = self.circuits.read().unwrap();
        circuits
            .iter()
            .filter(|(_, c)| c.state == CircuitState::Open)
            .map(|(domain, _)| domain.clone())
            .collect()
    }
}

impl Default for CircuitBreaker {
    fn default() -> Self {
        Self::new()
    }
}

/// Extract domain from URL for circuit breaker tracking
pub fn extract_domain(url: &str) -> Option<String> {
    url::Url::parse(url).ok().and_then(|u| u.host_str().map(String::from))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_circuit_starts_closed() {
        let breaker = CircuitBreaker::new();
        assert_eq!(breaker.can_request("example.com"), CircuitState::Closed);
    }

    #[test]
    fn test_circuit_opens_after_threshold() {
        let config = CircuitBreakerConfig {
            failure_threshold: 3,
            cooldown_duration: Duration::from_secs(60),
            success_reset_duration: Duration::from_secs(300),
        };
        let breaker = CircuitBreaker::with_config(config);

        // First 2 failures don't open
        breaker.record_failure("example.com");
        breaker.record_failure("example.com");
        assert_eq!(breaker.can_request("example.com"), CircuitState::Closed);

        // Third failure opens
        let opened = breaker.record_failure("example.com");
        assert!(opened);
        assert_eq!(breaker.can_request("example.com"), CircuitState::Open);
    }

    #[test]
    fn test_success_resets_failures() {
        let config = CircuitBreakerConfig {
            failure_threshold: 3,
            ..Default::default()
        };
        let breaker = CircuitBreaker::with_config(config);

        breaker.record_failure("example.com");
        breaker.record_failure("example.com");
        breaker.record_success("example.com");

        let (state, failures) = breaker.get_status("example.com");
        assert_eq!(state, CircuitState::Closed);
        assert_eq!(failures, 0);
    }

    #[test]
    fn test_extract_domain() {
        assert_eq!(
            extract_domain("https://example.com/path"),
            Some("example.com".to_string())
        );
        assert_eq!(
            extract_domain("http://sub.example.com:8080/path"),
            Some("sub.example.com".to_string())
        );
        assert_eq!(extract_domain("not-a-url"), None);
    }

    #[test]
    fn test_different_domains_independent() {
        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            ..Default::default()
        };
        let breaker = CircuitBreaker::with_config(config);

        // Open circuit for domain1
        breaker.record_failure("domain1.com");
        breaker.record_failure("domain1.com");
        assert_eq!(breaker.can_request("domain1.com"), CircuitState::Open);

        // domain2 should still be closed
        assert_eq!(breaker.can_request("domain2.com"), CircuitState::Closed);
    }

    #[test]
    fn test_get_open_circuits() {
        let config = CircuitBreakerConfig {
            failure_threshold: 1,
            ..Default::default()
        };
        let breaker = CircuitBreaker::with_config(config);

        breaker.record_failure("fail1.com");
        breaker.record_failure("fail2.com");
        breaker.record_success("success.com");

        let open = breaker.get_open_circuits();
        assert_eq!(open.len(), 2);
        assert!(open.contains(&"fail1.com".to_string()));
        assert!(open.contains(&"fail2.com".to_string()));
    }
}
