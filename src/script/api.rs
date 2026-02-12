//! JavaScript API bindings (ggg.*)
//!
//! This module will implement the JavaScript global API that scripts can use:
//! - ggg.on(eventName, callback, filter?) - Register event handlers
//! - ggg.log(message) - Logging from scripts
//! - ggg.config.get(key) - Access configuration
//!
//! Event object methods (attached to event context):
//! - e.setUrl(url) - Modify URL (beforeRequest)
//! - e.setHeader(key, value) - Set header (beforeRequest)
//! - e.setUserAgent(ua) - Set user agent (beforeRequest)
//! - e.rename(filename) - Rename file (completed)
//! - e.moveTo(path) - Move file (completed)
//!
//! Phase 3 implementation

// TODO: Implement JavaScript API bindings
