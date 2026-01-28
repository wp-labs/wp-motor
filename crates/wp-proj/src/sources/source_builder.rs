use wp_conf::connectors::{ParamMap, param_value_from_toml};
use wp_conf::sources::types::SourceItem;

/// Builder pattern for constructing SourceItem instances
///
/// This struct provides a fluent interface for creating SourceItem configurations
/// with method chaining. It simplifies the creation of data source configurations
/// by providing clear, descriptive methods for setting various properties.
///
pub struct SourceItemBuilder {
    /// Unique identifier for the source
    key: String,
    /// Connector type/name
    connect: String,
    /// Whether the source is enabled (Some(true) by default)
    enable: Option<bool>,
    /// List of tags for categorization
    tags: Vec<String>,
    /// Configuration parameters as key-value pairs
    params: ParamMap,
}

#[allow(dead_code)]
impl SourceItemBuilder {
    /// Creates a new SourceItemBuilder with the given key and connector type
    ///
    /// # Arguments
    /// * `key` - Unique identifier for the source
    /// * `connect` - Type/name of the connector to use
    ///
    /// # Returns
    /// A new SourceItemBuilder instance with default values:
    /// - enabled: true
    /// - tags: empty
    /// - params: empty
    pub fn new(key: &str, connect: &str) -> Self {
        Self {
            key: key.to_string(),
            connect: connect.to_string(),
            enable: Some(true), // Default to enabled
            tags: vec![],
            params: ParamMap::new(),
        }
    }

    /// Sets whether the source is enabled
    ///
    /// # Arguments
    /// * `enabled` - true to enable, false to disable
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enable = Some(enabled);
        self
    }

    /// Adds a single tag to the source
    ///
    /// Tags are used for categorizing and filtering sources.
    ///
    /// # Arguments
    /// * `tag` - Tag string to add
    pub fn tag(mut self, tag: &str) -> Self {
        self.tags.push(tag.to_string());
        self
    }

    /// Adds multiple tags to the source
    ///
    /// # Arguments
    /// * `tags` - Vector of tag strings to add
    pub fn tags(mut self, tags: Vec<&str>) -> Self {
        self.tags.extend(tags.into_iter().map(|t| t.to_string()));
        self
    }

    /// Adds a generic parameter with a TOML value
    ///
    /// # Arguments
    /// * `key` - Parameter name
    /// * `value` - TOML value
    pub fn param(mut self, key: &str, value: toml::Value) -> Self {
        self.params
            .insert(key.to_string(), param_value_from_toml(&value));
        self
    }

    /// Adds a string parameter
    ///
    /// # Arguments
    /// * `key` - Parameter name
    /// * `value` - String value
    pub fn param_str(mut self, key: &str, value: &str) -> Self {
        self.params.insert(
            key.to_string(),
            serde_json::Value::String(value.to_string()),
        );
        self
    }

    /// Adds an integer parameter
    ///
    /// # Arguments
    /// * `key` - Parameter name
    /// * `value` - Integer value
    pub fn param_int(mut self, key: &str, value: i64) -> Self {
        self.params.insert(
            key.to_string(),
            serde_json::Value::Number(serde_json::Number::from(value)),
        );
        self
    }

    /// Builds the final SourceItem
    ///
    /// Consumes the builder and returns a configured SourceItem instance.
    pub fn build(self) -> SourceItem {
        SourceItem {
            key: self.key,
            enable: self.enable,
            connect: self.connect,
            tags: self.tags,
            params: self.params,
        }
    }
}

/// Convenience constructors for commonly used source types
///
/// This module provides helper functions for creating common source configurations
/// with sensible defaults, reducing boilerplate code when setting up standard sources.
pub mod source_builders {
    use super::*;

    /// Creates a file-based data source
    ///
    /// Creates a source that reads from a file with text encoding.
    /// The source reads data line by line from the specified file.
    ///
    /// # Arguments
    /// * `key` - Unique identifier for the source
    /// * `file` - Path to the file to read from
    ///
    /// # Returns
    /// A configured SourceItem for file reading
    pub fn file_source(key: &str, file: &str) -> SourceItem {
        SourceItemBuilder::new(key, "file_src")
            .param_str("file", file)
            .param_str("encode", "text")
            .build()
    }

    /// Creates an in-memory channel source used by WPL send_to_src / split_to_src
    pub fn channel_source(key: &str, capacity: i64) -> SourceItem {
        SourceItemBuilder::new(key, "channel_src")
            .param_int("capacity", capacity)
            .build()
    }

    /// Creates a UDP syslog receiver source
    ///
    /// Creates a source that listens for syslog messages over UDP.
    /// Suitable for high-throughput logging where message loss is acceptable.
    ///
    /// # Arguments
    /// * `key` - Unique identifier for the source
    /// * `addr` - IP address to bind to (e.g., "0.0.0.0")
    /// * `port` - Port number to listen on
    ///
    /// # Returns
    /// A configured SourceItem for UDP syslog reception
    pub fn syslog_udp_source(key: &str, addr: &str, port: i64) -> SourceItem {
        SourceItemBuilder::new(key, "syslog_udp_src")
            .param_str("addr", addr)
            .param_int("port", port)
            .param_str("protocol", "udp")
            .build()
    }

    /// Creates a TCP syslog receiver source
    ///
    /// Creates a source that listens for syslog messages over TCP.
    /// Suitable for reliable log delivery where message integrity is important.
    ///
    /// # Arguments
    /// * `key` - Unique identifier for the source
    /// * `addr` - IP address to bind to (e.g., "0.0.0.0")
    /// * `port` - Port number to listen on
    ///
    /// # Returns
    /// A configured SourceItem for TCP syslog reception
    #[allow(dead_code)]
    pub fn syslog_tcp_source(key: &str, addr: &str, port: i64) -> SourceItem {
        SourceItemBuilder::new(key, "syslog_tcp_src")
            .param_str("addr", addr)
            .param_int("port", port)
            .param_str("protocol", "tcp")
            .build()
    }

    /// Creates a generic SourceItem builder
    ///
    /// Returns a fresh SourceItemBuilder for creating custom source configurations.
    ///
    /// # Arguments
    /// * `key` - Unique identifier for the source
    /// * `connect` - Connector type/name
    ///
    /// # Returns
    /// A new SourceItemBuilder instance
    #[allow(dead_code)]
    pub fn builder(key: &str, connect: &str) -> SourceItemBuilder {
        SourceItemBuilder::new(key, connect)
    }
}

#[cfg(test)]
mod tests {
    use super::source_builders;

    #[test]
    fn file_source_sets_expected_fields() {
        let item = source_builders::file_source("file_1", "gen.dat");
        assert_eq!(item.key, "file_1");
        assert_eq!(item.connect, "file_src");
        assert_eq!(item.params.get("file").unwrap().as_str(), Some("gen.dat"));
    }

    #[test]
    fn syslog_builder_respects_protocol() {
        let item = source_builders::syslog_udp_source("syslog_1", "0.0.0.0", 9000);
        assert_eq!(item.params.get("protocol").unwrap().as_str(), Some("udp"));
        assert_eq!(item.params.get("port").unwrap().as_i64(), Some(9000));
    }
}
