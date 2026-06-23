//! Error type for the plugin SDK surface.

/// Error raised by a [`crate::Plugin`] method or a proto-to-native decode.
#[derive(Debug, thiserror::Error)]
pub enum PluginError {
    /// The plugin does not implement the requested method.
    #[error("plugin does not implement this method")]
    Unsupported,
    /// A proto message failed to decode into its native form.
    #[error("proto decode failed: {0}")]
    Decode(#[from] prost::DecodeError),
    /// The plugin reported a structured op-level failure.
    #[error("plugin op error [{code}] {message}")]
    Op {
        /// Machine-readable error code (e.g. `"INVALID_INPUT"`).
        code: String,
        /// Human-readable message.
        message: String,
    },
    /// Any other failure, carried as a free-form message.
    #[error("{0}")]
    Other(String),
}

/// Result alias for fallible SDK operations.
pub type Result<T> = core::result::Result<T, PluginError>;
