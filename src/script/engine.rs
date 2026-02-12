use crate::script::error::{ScriptError, ScriptResult};
use crate::script::events::{EventContext, HookEvent};
use regex::Regex;
use rustyscript::{Runtime, RuntimeOptions};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// JavaScript engine wrapper around rustyscript Runtime
///
/// Manages:
/// - rustyscript Runtime initialization
/// - Event handler registry
/// - Handler execution with timeout enforcement
/// - URL filtering
pub struct ScriptEngine {
    runtime: Runtime,
    handlers: Arc<Mutex<HashMap<HookEvent, Vec<EventHandler>>>>,
    #[allow(dead_code)]
    timeout: Duration,
}

/// Registered event handler
#[derive(Debug, Clone)]
struct EventHandler {
    /// Function name in JavaScript (generated callback ID)
    callback_id: String,
    /// URL filter pattern (optional)
    filter: Option<UrlFilter>,
    /// Source script path for error reporting
    script_path: PathBuf,
}

/// URL filter for conditional handler execution
#[derive(Debug, Clone)]
struct UrlFilter {
    #[allow(dead_code)]
    pattern: String,
    regex: Regex,
}

impl UrlFilter {
    /// Create new URL filter from pattern string
    fn new(pattern: String) -> ScriptResult<Self> {
        // Convert simple patterns to regex
        let regex_pattern = if pattern.contains('*') || pattern.contains('^') || pattern.contains('$')
        {
            // Already looks like regex
            pattern.clone()
        } else {
            // Simple substring match - escape and wrap
            regex::escape(&pattern)
        };

        let regex = Regex::new(&regex_pattern).map_err(|_| {
            ScriptError::InvalidFilter {
                script: "unknown".to_string(),
                pattern: pattern.clone(),
            }
        })?;

        Ok(Self { pattern, regex })
    }

    /// Check if URL matches this filter
    fn matches(&self, url: &str) -> bool {
        self.regex.is_match(url)
    }
}

impl ScriptEngine {
    /// Create new script engine with timeout
    /// Clear all registered handlers (used when reloading scripts)
    pub fn clear_handlers(&mut self) {
        let mut handlers = self.handlers.lock().unwrap();
        handlers.clear();
        drop(handlers);

        // Also clear JavaScript-side handler registry and reset callback ID
        if let Err(e) = self.runtime.eval::<()>("ggg._handlers.clear(); ggg._nextCallbackId = 0;") {
            tracing::warn!("Failed to clear JavaScript handlers: {}", e);
        }

        tracing::debug!("Cleared all script handlers");
    }

    pub fn new(timeout: Duration) -> ScriptResult<Self> {
        // Initialize rustyscript runtime with options
        let options = RuntimeOptions {
            timeout,
            ..Default::default()
        };

        let mut runtime = Runtime::new(options).map_err(|e| {
            ScriptError::RuntimeInitError(format!("Failed to create runtime: {}", e))
        })?;

        let handlers = Arc::new(Mutex::new(HashMap::new()));
        let handlers_clone = Arc::clone(&handlers);

        // Register global `ggg` object with API functions
        let register_code = r#"
            globalThis.ggg = {
                // Handler storage (populated from Rust)
                _handlers: new Map(),
                _nextCallbackId: 0,

                // Register event handler
                on: function(eventName, callback, filter) {
                    if (typeof callback !== 'function') {
                        throw new Error('Callback must be a function');
                    }

                    // Generate unique callback ID
                    const callbackId = `__callback_${this._nextCallbackId++}`;
                    globalThis[callbackId] = callback;

                    // Store handler info for Rust to retrieve
                    if (!this._handlers.has(eventName)) {
                        this._handlers.set(eventName, []);
                    }
                    this._handlers.get(eventName).push({
                        callbackId: callbackId,
                        filter: filter || null
                    });

                    return true;
                },

                // Logging function (buffered, flushed to tracing by Rust)
                _logBuffer: [],
                log: function(message) {
                    ggg._logBuffer.push(String(message));
                },

                // Config access (stub for now)
                config: {
                    get: function(key) {
                        return undefined;
                    }
                }
            };

            // Override console methods to redirect output to ggg._logBuffer
            // Prevents Deno core console from writing directly to stdout
            globalThis.console = {
                log: function(...args) {
                    ggg._logBuffer.push(args.map(String).join(' '));
                },
                warn: function(...args) {
                    ggg._logBuffer.push('[WARN] ' + args.map(String).join(' '));
                },
                error: function(...args) {
                    ggg._logBuffer.push('[ERROR] ' + args.map(String).join(' '));
                },
                info: function(...args) {
                    ggg._logBuffer.push(args.map(String).join(' '));
                },
                debug: function(...args) {
                    ggg._logBuffer.push('[DEBUG] ' + args.map(String).join(' '));
                },
            };
        "#;

        runtime.eval::<()>(register_code).map_err(|e| {
            ScriptError::RuntimeInitError(format!("Failed to register ggg API: {}", e))
        })?;

        Ok(Self {
            runtime,
            handlers: handlers_clone,
            timeout,
        })
    }

    /// Load and compile a script file
    pub fn load_script(&mut self, path: &Path) -> ScriptResult<()> {
        // Read script file
        let script_content = std::fs::read_to_string(path).map_err(|e| ScriptError::FileReadError {
            path: path.to_owned(),
            source: e,
        })?;

        // Execute script to register handlers
        self.runtime
            .eval::<()>(&script_content)
            .map_err(|e| ScriptError::CompilationError {
                path: path.to_owned(),
                message: e.to_string(),
            })?;

        // Extract registered handlers from JavaScript
        let handlers_json: String = self
            .runtime
            .eval("JSON.stringify(Array.from(ggg._handlers.entries()))")
            .map_err(|e| ScriptError::InternalError(format!("Failed to get handlers: {}", e)))?;

        // Parse handlers and store in registry
        let handlers_data: Vec<(String, Vec<serde_json::Value>)> =
            serde_json::from_str(&handlers_json)
                .map_err(|e| ScriptError::InternalError(format!("Failed to parse handlers: {}", e)))?;

        let mut registry = self.handlers.lock().unwrap();

        for (event_name, handlers_list) in handlers_data {
            let event = HookEvent::from_str(&event_name).ok_or_else(|| {
                ScriptError::InvalidEventName(event_name.clone())
            })?;

            let event_handlers = registry.entry(event).or_insert_with(Vec::new);

            for handler_data in handlers_list {
                let callback_id = handler_data["callbackId"]
                    .as_str()
                    .ok_or_else(|| ScriptError::InternalError("Missing callbackId".to_string()))?
                    .to_string();

                let filter = if let Some(filter_str) = handler_data["filter"].as_str() {
                    Some(UrlFilter::new(filter_str.to_string())?)
                } else {
                    None
                };

                event_handlers.push(EventHandler {
                    callback_id,
                    filter,
                    script_path: path.to_owned(),
                });
            }
        }

        // Clear JavaScript handlers map for next script
        // (Callbacks remain in globalThis, handlers map is just for registration)
        self.runtime.eval::<()>("ggg._handlers.clear()").map_err(|e| {
            ScriptError::InternalError(format!("Failed to clear handlers map: {}", e))
        })?;

        tracing::info!("Loaded script: {:?}", path);
        Ok(())
    }

    /// Execute handlers for a specific event
    pub fn execute_handlers<C: EventContext>(
        &mut self,
        event: HookEvent,
        ctx: &mut C,
        effective_script_files: &std::collections::HashMap<String, bool>,
    ) -> ScriptResult<bool> {
        let handlers = self.handlers.lock().unwrap();
        let event_handlers = match handlers.get(&event) {
            Some(h) if !h.is_empty() => h.clone(),
            _ => return Ok(true), // No handlers, continue
        };
        drop(handlers); // Release lock

        // Execute each handler in order
        for handler in event_handlers {
            // Check if script file is enabled (default to enabled if not in map)
            let filename = handler
                .script_path
                .file_name()
                .and_then(|f| f.to_str())
                .unwrap_or("");
            let is_enabled = effective_script_files.get(filename).copied().unwrap_or(true);

            if !is_enabled {
                tracing::debug!(
                    script = ?handler.script_path,
                    "Skipping disabled script: {}",
                    filename
                );
                continue;
            }

            // Serialize current context to JSON (updated after each handler)
            let ctx_json = ctx.to_json()?;

            // Check URL filter if present
            if let Some(ref filter) = handler.filter {
                if let Some(url) = ctx_json.get("url").and_then(|v| v.as_str()) {
                    if !filter.matches(url) {
                        continue; // Skip this handler
                    }
                }
            }

            // Execute handler with timeout
            let callback_code = format!(
                "(function() {{
                    const ctx = {};
                    const result = {}(ctx);
                    return {{ result: result, ctx: ctx }};
                }})()",
                serde_json::to_string(&ctx_json)?,
                handler.callback_id
            );

            let result: serde_json::Value = match self.runtime.eval(&callback_code) {
                Ok(v) => v,
                Err(e) => {
                    tracing::error!(
                        script = ?handler.script_path,
                        "Script execution error: {}",
                        e
                    );
                    self.flush_log_buffer(&handler.script_path);
                    continue; // Continue to next handler on error
                }
            };

            // Flush ggg.log() messages to tracing
            self.flush_log_buffer(&handler.script_path);

            // Update context from modified JavaScript object
            if let Some(modified_ctx) = result.get("ctx") {
                *ctx = C::from_json(modified_ctx.clone())?;
            }

            // Check if handler returned false (stop propagation)
            if let Some(handler_result) = result.get("result") {
                if handler_result.is_boolean() && !handler_result.as_bool().unwrap() {
                    tracing::debug!(
                        event = ?event,
                        script = ?handler.script_path,
                        "Handler stopped propagation"
                    );
                    return Ok(false); // Stop processing
                }
            }
        }

        Ok(true) // Continue processing
    }

    /// Flush buffered ggg.log() messages to tracing
    fn flush_log_buffer(&mut self, script_path: &Path) {
        let messages: Vec<String> = self
            .runtime
            .eval("ggg._logBuffer.splice(0)")
            .unwrap_or_default();
        for msg in messages {
            tracing::info!(script = ?script_path, "[Script] {}", msg);
        }
    }

    /// Get handler count for an event (for testing)
    #[cfg(test)]
    pub fn handler_count(&self, event: HookEvent) -> usize {
        self.handlers
            .lock()
            .unwrap()
            .get(&event)
            .map(|h| h.len())
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::script::events::BeforeRequestContext;
    use std::collections::HashMap;

    #[test]
    fn test_engine_creation() {
        let engine = ScriptEngine::new(Duration::from_secs(30));
        assert!(engine.is_ok());
    }

    #[test]
    fn test_url_filter_simple_match() {
        let filter = UrlFilter::new("pbs.twimg.com".to_string()).unwrap();
        assert!(filter.matches("https://pbs.twimg.com/media/image.jpg"));
        assert!(!filter.matches("https://example.com/file.zip"));
    }

    #[test]
    fn test_url_filter_regex_match() {
        let filter = UrlFilter::new("^https://.*\\.twimg\\.com".to_string()).unwrap();
        assert!(filter.matches("https://pbs.twimg.com/media/image.jpg"));
        assert!(filter.matches("https://video.twimg.com/video.mp4"));
        assert!(!filter.matches("http://pbs.twimg.com/image.jpg"));
    }

    #[test]
    fn test_load_simple_script() {
        let mut engine = ScriptEngine::new(Duration::from_secs(30)).unwrap();

        // Create a test script
        let test_script = r#"
            ggg.on('beforeRequest', function(e) {
                ggg.log('Handler called');
                return true;
            });
        "#;

        // Write to temp file
        let temp_dir = std::env::temp_dir();
        let script_path = temp_dir.join("test_script.js");
        std::fs::write(&script_path, test_script).unwrap();

        // Load script
        let result = engine.load_script(&script_path);
        assert!(result.is_ok(), "Failed to load script: {:?}", result);

        // Verify handler was registered
        assert_eq!(engine.handler_count(HookEvent::BeforeRequest), 1);

        // Cleanup
        std::fs::remove_file(script_path).ok();
    }

    #[test]
    fn test_execute_handler_modifies_context() {
        let mut engine = ScriptEngine::new(Duration::from_secs(30)).unwrap();

        let test_script = r#"
            ggg.on('beforeRequest', function(e) {
                e.url = 'https://modified.com/file.zip';
                e.headers['X-Custom'] = 'test';
                return true;
            });
        "#;

        let temp_dir = std::env::temp_dir();
        let script_path = temp_dir.join("test_modify.js");
        std::fs::write(&script_path, test_script).unwrap();

        engine.load_script(&script_path).unwrap();

        // Create context
        let mut ctx = BeforeRequestContext {
            url: "https://example.com/file.zip".to_string(),
            headers: HashMap::new(),
            user_agent: None,
            download_id: None,
        };

        // Execute handlers
        let script_files = std::collections::HashMap::new();
        let result = engine.execute_handlers(HookEvent::BeforeRequest, &mut ctx, &script_files);
        assert!(result.is_ok());
        assert!(result.unwrap()); // Should continue

        // Verify modifications
        assert_eq!(ctx.url, "https://modified.com/file.zip");
        assert_eq!(ctx.headers.get("X-Custom"), Some(&"test".to_string()));

        std::fs::remove_file(script_path).ok();
    }

    #[test]
    fn test_handler_stop_propagation() {
        let mut engine = ScriptEngine::new(Duration::from_secs(30)).unwrap();

        let test_script = r#"
            ggg.on('beforeRequest', function(e) {
                ggg.log('First handler');
                return false; // Stop propagation
            });

            ggg.on('beforeRequest', function(e) {
                ggg.log('Second handler - should not run');
                e.url = 'https://modified.com';
                return true;
            });
        "#;

        let temp_dir = std::env::temp_dir();
        let script_path = temp_dir.join("test_stop.js");
        std::fs::write(&script_path, test_script).unwrap();

        engine.load_script(&script_path).unwrap();

        let mut ctx = BeforeRequestContext {
            url: "https://example.com/file.zip".to_string(),
            headers: HashMap::new(),
            user_agent: None,
            download_id: None,
        };

        let script_files = std::collections::HashMap::new();
        let result = engine.execute_handlers(HookEvent::BeforeRequest, &mut ctx, &script_files);
        assert!(result.is_ok());
        assert!(!result.unwrap()); // Should stop

        // URL should NOT be modified
        assert_eq!(ctx.url, "https://example.com/file.zip");

        std::fs::remove_file(script_path).ok();
    }

    #[test]
    fn test_url_filter_conditional_execution() {
        let mut engine = ScriptEngine::new(Duration::from_secs(30)).unwrap();

        let test_script = r#"
            ggg.on('beforeRequest', function(e) {
                e.headers['X-Twitter'] = 'yes';
                return true;
            }, 'twimg.com');
        "#;

        let temp_dir = std::env::temp_dir();
        let script_path = temp_dir.join("test_filter.js");
        std::fs::write(&script_path, test_script).unwrap();

        engine.load_script(&script_path).unwrap();

        // Test with matching URL
        let mut ctx1 = BeforeRequestContext {
            url: "https://pbs.twimg.com/image.jpg".to_string(),
            headers: HashMap::new(),
            user_agent: None,
            download_id: None,
        };

        let script_files = HashMap::new();
        engine
            .execute_handlers(HookEvent::BeforeRequest, &mut ctx1, &script_files)
            .unwrap();
        assert_eq!(ctx1.headers.get("X-Twitter"), Some(&"yes".to_string()));

        // Test with non-matching URL
        let mut ctx2 = BeforeRequestContext {
            url: "https://example.com/file.zip".to_string(),
            headers: HashMap::new(),
            user_agent: None,
            download_id: None,
        };

        engine
            .execute_handlers(HookEvent::BeforeRequest, &mut ctx2, &script_files)
            .unwrap();
        assert_eq!(ctx2.headers.get("X-Twitter"), None); // Should not be set

        std::fs::remove_file(script_path).ok();
    }
}
