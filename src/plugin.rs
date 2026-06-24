//! The `Plugin` trait: the contract every plugin implements.

use std::collections::HashMap;

use crate::error::{PluginError, Result};
use crate::proto::plugin as proto;
use crate::protocol::{DispatchContext, ParamValue, PollOutcome};

/// Payload variant for [`Plugin::update_assembly`].
///
/// Mirrors the proto `UpdateAssemblyRequest.payload` oneof. `Full` is
/// a fresh assembly snapshot; `Delta` is a delta edit list (decode
/// via molex's `serialize_edits` / `deserialize_edits` pair).
#[derive(Debug, Clone, Copy)]
pub enum AssemblyPayload<'a> {
    /// Fresh assembly snapshot; replaces the plugin's view.
    Full(&'a [u8]),
    /// Delta edit list; applied incrementally on top of the
    /// plugin's current view.
    Delta(&'a [u8]),
}

impl<'a> AssemblyPayload<'a> {
    /// Borrow the payload bytes regardless of variant.
    #[must_use]
    pub fn bytes(&self) -> &'a [u8] {
        match self {
            AssemblyPayload::Full(b) | AssemblyPayload::Delta(b) => b,
        }
    }

    /// True if the payload is a delta edit list.
    #[must_use]
    pub fn is_delta(&self) -> bool {
        matches!(self, AssemblyPayload::Delta(_))
    }
}

/// Worker-side plugin interface.
///
/// Both Python and native plugins implement this; the host dispatches
/// to it through the C ABI or the in-process boundary.
///
/// Concurrency: methods take `&self` because plugin state mutation is
/// done through interior-mutable host runtimes (Python's GIL,
/// thread-locked C++ FFI). Plugins MUST be `Send` so they can be moved
/// between worker startup and the request-loop thread.
pub trait Plugin: Send {
    /// Start a session with the given canonical Assembly. Returns the
    /// SessionId chosen by the plugin and the assembly bytes of the
    /// assembly the plugin actually settled on after any post-Init
    /// normalization (full-atom pose build, hydrogen fill, terminal O,
    /// etc.). Plugins with no normalization step return an empty
    /// `Vec<u8>`; the host then keeps its input assembly.
    ///
    /// `params` is the generic puzzle-config channel (weight-patch +
    /// objective-filter entries); plugins that don't consume it ignore it.
    ///
    /// # Errors
    ///
    /// Returns an error if the plugin can't ingest the assembly or
    /// allocate a session.
    fn init(
        &self,
        assembly_bytes: &[u8],
        params: &HashMap<String, ParamValue>,
    ) -> Result<(u64, Vec<u8>)>;

    /// Return the plugin's op + query catalog.
    ///
    /// # Errors
    ///
    /// Returns an error if the plugin can't produce its registration.
    fn register(&self) -> Result<proto::PluginRegistration>;

    /// Push an Assembly update to a session. `payload` carries either
    /// a fresh assembly snapshot (`Full`) or a delta edit list
    /// (`Delta`); `from_gen`/`to_gen` are the host's broadcast
    /// generation counters. A plugin whose local gen doesn't match
    /// `from_gen` should arm a `STALE_GEN` error to return on its
    /// next dispatch so the host re-syncs.
    ///
    /// # Errors
    ///
    /// Returns an error if the payload can't be applied or the session
    /// is unknown.
    fn update_assembly(
        &self,
        session: u64,
        payload: AssemblyPayload<'_>,
        from_gen: u64,
        to_gen: u64,
    ) -> Result<()>;

    /// Tear down a session. Idempotent.
    ///
    /// # Errors
    ///
    /// Returns an error if the plugin can't release the session state.
    fn drop_session(&self, session: u64) -> Result<()>;

    /// Single-shot mutating op. Returns the plugin's working Assembly
    /// post-op (assembly bytes); the orchestrator copies locked-entity
    /// slices into canonical state.
    ///
    /// # Errors
    ///
    /// Default impl returns [`PluginError::Unsupported`]. Implementations
    /// return an error on op-level failure.
    fn invoke(
        &self,
        _session: u64,
        _op: &str,
        _ctx: &DispatchContext,
        _params: &HashMap<String, ParamValue>,
    ) -> Result<Vec<u8>> {
        Err(PluginError::Unsupported)
    }

    /// Begin a long-running op under the host-assigned `request_id`. The
    /// plugin keys its stream state on that id.
    ///
    /// # Errors
    ///
    /// Default impl returns [`PluginError::Unsupported`]. Implementations
    /// return an error if the stream can't be started.
    // Arg list is the streaming-dispatch ABI contract (session/op/ctx/params/
    // request_id); it mirrors the C-ABI and must not be refactored away.
    #[allow(clippy::too_many_arguments)]
    fn start_stream(
        &self,
        _session: u64,
        _op: &str,
        _ctx: &DispatchContext,
        _params: &HashMap<String, ParamValue>,
        _request_id: u64,
    ) -> Result<()> {
        Err(PluginError::Unsupported)
    }

    /// Return the latest snapshot for a running stream.
    ///
    /// # Errors
    ///
    /// Default impl returns [`PluginError::Unsupported`]. Op-level failure
    /// surfaces as `PollOutcome::Error` rather than `Err`.
    fn poll_stream(&self, _request_id: u64) -> Result<PollOutcome> {
        Err(PluginError::Unsupported)
    }

    /// Push new params to a running stream.
    ///
    /// # Errors
    ///
    /// Default impl returns [`PluginError::Unsupported`]. Implementations
    /// return an error if the request id is unknown.
    fn update_stream(&self, _request_id: u64, _params: &HashMap<String, ParamValue>) -> Result<()> {
        Err(PluginError::Unsupported)
    }

    /// Stop a running stream. Idempotent.
    ///
    /// # Errors
    ///
    /// Default impl returns [`PluginError::Unsupported`]. Implementations
    /// return an error if cleanup fails.
    fn cancel_stream(&self, _request_id: u64) -> Result<()> {
        Err(PluginError::Unsupported)
    }

    /// Single-shot read query. Returns query-defined opaque bytes.
    ///
    /// `assembly` (when non-empty) names a specific composition to
    /// read/score instead of the session pose; an empty slice means the
    /// query operates on the live session / its in-flight snapshot.
    ///
    /// # Errors
    ///
    /// Default impl returns [`PluginError::Unsupported`]. Implementations
    /// return an error on query failure.
    // Arg list mirrors the C-ABI `query` contract (session/query/ctx/params/
    // assembly); it must stay in lockstep with the vtable signature.
    #[allow(clippy::too_many_arguments)]
    fn query(
        &self,
        _session: u64,
        _query: &str,
        _ctx: &DispatchContext,
        _params: &HashMap<String, ParamValue>,
        _assembly: &[u8],
    ) -> Result<Vec<u8>> {
        Err(PluginError::Unsupported)
    }
}
