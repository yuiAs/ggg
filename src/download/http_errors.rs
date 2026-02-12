/// HTTP error category for user-facing messages
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpErrorCategory {
    Network,    // Connection errors (no status code)
    Client,     // 4xx errors
    Server,     // 5xx errors
    Auth,       // 401, 403
    RateLimit,  // 429
}

/// Enriched HTTP error information
#[derive(Debug, Clone)]
pub struct HttpErrorInfo {
    pub status_code: Option<u16>,
    pub category: HttpErrorCategory,
    pub description: String,
    pub suggestion: String,
    pub is_retryable: bool,
}

impl HttpErrorInfo {
    /// Create from HTTP status code
    pub fn from_status(status: u16) -> Self {
        match status {
            400 => Self {
                status_code: Some(400),
                category: HttpErrorCategory::Client,
                description: "Bad Request".to_string(),
                suggestion: "The URL or request format is invalid. Check the download URL.".to_string(),
                is_retryable: false,
            },
            401 => Self {
                status_code: Some(401),
                category: HttpErrorCategory::Auth,
                description: "Unauthorized".to_string(),
                suggestion: "Authentication required. Use authRequired hook to provide credentials.".to_string(),
                is_retryable: false,
            },
            403 => Self {
                status_code: Some(403),
                category: HttpErrorCategory::Auth,
                description: "Forbidden".to_string(),
                suggestion: "Access denied. Try a different User-Agent or check permissions.".to_string(),
                is_retryable: false,
            },
            404 => Self {
                status_code: Some(404),
                category: HttpErrorCategory::Client,
                description: "Not Found".to_string(),
                suggestion: "The file no longer exists at this URL.".to_string(),
                is_retryable: false,
            },
            410 => Self {
                status_code: Some(410),
                category: HttpErrorCategory::Client,
                description: "Gone".to_string(),
                suggestion: "The file has been permanently removed.".to_string(),
                is_retryable: false,
            },
            429 => Self {
                status_code: Some(429),
                category: HttpErrorCategory::RateLimit,
                description: "Too Many Requests".to_string(),
                suggestion: "Rate limited. Retry will happen automatically with delay.".to_string(),
                is_retryable: true,
            },
            500 => Self {
                status_code: Some(500),
                category: HttpErrorCategory::Server,
                description: "Internal Server Error".to_string(),
                suggestion: "Server-side issue. Retry may succeed.".to_string(),
                is_retryable: true,
            },
            502 => Self {
                status_code: Some(502),
                category: HttpErrorCategory::Server,
                description: "Bad Gateway".to_string(),
                suggestion: "Server connection issue. Retry may succeed.".to_string(),
                is_retryable: true,
            },
            503 => Self {
                status_code: Some(503),
                category: HttpErrorCategory::Server,
                description: "Service Unavailable".to_string(),
                suggestion: "Server temporarily unavailable. Retry will happen automatically.".to_string(),
                is_retryable: true,
            },
            504 => Self {
                status_code: Some(504),
                category: HttpErrorCategory::Server,
                description: "Gateway Timeout".to_string(),
                suggestion: "Server response timeout. Retry may succeed.".to_string(),
                is_retryable: true,
            },
            // Generic fallbacks
            _ if status >= 400 && status < 500 => Self {
                status_code: Some(status),
                category: HttpErrorCategory::Client,
                description: format!("Client Error ({})", status),
                suggestion: "Check the request details and URL.".to_string(),
                is_retryable: false,
            },
            _ if status >= 500 => Self {
                status_code: Some(status),
                category: HttpErrorCategory::Server,
                description: format!("Server Error ({})", status),
                suggestion: "Server-side issue. Retry may help.".to_string(),
                is_retryable: true,
            },
            _ => Self {
                status_code: Some(status),
                category: HttpErrorCategory::Client,
                description: format!("HTTP Error ({})", status),
                suggestion: "Unknown error. Check logs for details.".to_string(),
                is_retryable: false,
            },
        }
    }

    /// Create for network errors (no status code)
    pub fn network_error(message: &str) -> Self {
        Self {
            status_code: None,
            category: HttpErrorCategory::Network,
            description: "Network Error".to_string(),
            suggestion: format!("Connection failed: {}. Check network connectivity.", message),
            is_retryable: true,
        }
    }

    /// Format for display
    pub fn format(&self) -> String {
        if let Some(code) = self.status_code {
            format!("HTTP {} - {}", code, self.description)
        } else {
            self.description.clone()
        }
    }

    /// Get category icon emoji
    pub fn category_icon(&self) -> &str {
        match self.category {
            HttpErrorCategory::Network => "🌐",
            HttpErrorCategory::Client => "❌",
            HttpErrorCategory::Server => "⚠️",
            HttpErrorCategory::Auth => "🔒",
            HttpErrorCategory::RateLimit => "⏱️",
        }
    }
}
