//! Native-Rust mirrors of the `proto::plugin` protocol types used by the
//! [`crate::Plugin`] trait surface.

use std::collections::HashMap;

#[cfg(feature = "python")]
use pyo3::prelude::*;

use crate::proto::plugin::ScoreReport;

/// Reference to a specific residue inside an entity. Mirror of
/// `proto::plugin::ResidueRef`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(
    feature = "python",
    pyclass(name = "ResidueRef", module = "foldit_plugin_sdk", from_py_object)
)]
pub struct ResidueRef {
    /// Entity the residue belongs to.
    pub entity_id: molex::EntityId,
    /// 0-indexed within entity.
    pub residue_index: u32,
}

#[cfg(feature = "python")]
#[pymethods]
impl ResidueRef {
    /// Raw entity id (`u32`) the residue belongs to.
    #[must_use]
    #[getter]
    pub fn entity_id(&self) -> u32 {
        self.entity_id.raw()
    }

    /// 0-indexed residue position within the entity.
    #[must_use]
    #[getter]
    pub fn residue_index(&self) -> u32 {
        self.residue_index
    }
}

/// Captured by the orchestrator at op-trigger time and frozen on the wire.
/// Streams do NOT receive an updated context mid-flight; only
/// `UpdateStream(params)` can change values during a running stream.
#[derive(Debug, Clone, Default)]
#[cfg_attr(
    feature = "python",
    pyclass(name = "DispatchContext", module = "foldit_plugin_sdk", from_py_object)
)]
pub struct DispatchContext {
    /// Entity the user has currently focused, or `None` for session
    /// mode (no specific entity targeted).
    pub focused_entity_id: Option<molex::EntityId>,
    /// Residue selection at op-trigger time.
    pub selection: Vec<ResidueRef>,
    /// Residues the plugin may redesign (change identity at), the puzzle's
    /// design mask. Carried alongside the selection so the engine can gate
    /// identity changes; orthogonal to `selection`, which says where to
    /// operate. Empty when the session gates no design.
    pub designable: Vec<ResidueRef>,
}

#[cfg(feature = "python")]
#[pymethods]
impl DispatchContext {
    /// Raw id (`u32`) of the focused entity, or `None` in session mode.
    #[must_use]
    #[getter]
    pub fn focused_entity_id(&self) -> Option<u32> {
        self.focused_entity_id.map(molex::EntityId::raw)
    }

    /// Residue selection at op-trigger time.
    #[must_use]
    #[getter]
    pub fn selection(&self) -> Vec<ResidueRef> {
        self.selection.clone()
    }

    /// Residues the plugin may redesign (the puzzle's design mask).
    #[must_use]
    #[getter]
    pub fn designable(&self) -> Vec<ResidueRef> {
        self.designable.clone()
    }
}

/// Native-Rust parameter value.
///
/// Mirrors `proto::plugin::ParamValue` (a oneof on the wire) but in
/// ergonomic enum form for the `Plugin` trait surface. Wire-native
/// conversion happens at the IPC boundary.
#[derive(Debug, Clone, PartialEq)]
pub enum ParamValue {
    /// 32-bit signed integer.
    Int(i32),
    /// 32-bit float.
    Float(f32),
    /// Boolean.
    Bool(bool),
    /// Used for both `PARAM_TYPE_STRING` and `PARAM_TYPE_ENUM`.
    String(String),
    /// 3-component float vector (positions, axes, etc.).
    Vec3([f32; 3]),
}

/// Outcome of a `PollStream` call. Native-Rust mirror of
/// `proto::plugin::PollStreamResponse`'s oneof.
#[derive(Debug, Clone)]
pub enum PollOutcome {
    /// Stream still running. `latest_assembly` is the working state at
    /// poll time; not authoritative until promoted on `Final` /
    /// `Cancelled`.
    Pending {
        /// Working assembly snapshot, if the plugin emits one.
        latest_assembly: Option<Vec<u8>>,
        /// Progress fraction in `[0.0, 1.0]`, if the plugin tracks it.
        progress: Option<f32>,
        /// Human-readable stage label, if provided.
        stage: Option<String>,
        /// Warm score of `latest_assembly`, if the plugin scores it.
        score: Option<ScoreReport>,
    },
    /// Accepted intermediate the host should commit into canonical state
    /// while the stream keeps running. Same payload as `Pending`, but the
    /// host commits it rather than treating it as a discardable preview;
    /// unlike a terminal it does not end the op (more checkpoints or a
    /// terminal follow), so the poll loop keeps going.
    Checkpoint {
        /// Working assembly snapshot, if the plugin emits one.
        latest_assembly: Option<Vec<u8>>,
        /// Progress fraction in `[0.0, 1.0]`, if the plugin tracks it.
        progress: Option<f32>,
        /// Human-readable stage label, if provided.
        stage: Option<String>,
        /// Warm score of `latest_assembly`, if the plugin scores it.
        score: Option<ScoreReport>,
    },
    /// Stream stopped at host request (the host sent `CancelStream` and
    /// the plugin returned its working pose). Same downstream handling
    /// as `Final`: the orchestrator promotes `assembly` into canonical
    /// state. Distinguished from `Final` only so the host can tell
    /// "the algorithm reached its endpoint" from "the user asked it
    /// to stop"; for open-ended ops (wiggle, shake) this IS the
    /// success terminal.
    Cancelled {
        /// Working assembly bytes for the orchestrator to promote.
        assembly: Vec<u8>,
        /// Warm score of `assembly`, if the plugin scores it.
        score: Option<ScoreReport>,
    },
    /// Stream finished successfully. `assembly` is the definitive output
    /// the orchestrator promotes into canonical state.
    Final {
        /// Definitive assembly bytes for the orchestrator to promote.
        assembly: Vec<u8>,
        /// Warm score of `assembly`, if the plugin scores it.
        score: Option<ScoreReport>,
    },
    /// Op-level failure. Distinct from a transport-level error.
    Error {
        /// Machine-readable error code (e.g. `"STREAM_INTERNAL"`).
        code: String,
        /// Human-readable error message.
        message: String,
        /// Optional structured detail map.
        details: HashMap<String, String>,
    },
}
