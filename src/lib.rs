//! Foldit plugin SDK.
//!
//! Owns the plugin protocol (`plugin.proto` compiled via prost), the
//! protocol types, and the `Plugin` trait. Exposes a cbindgen C-ABI for the
//! rosetta C++ bridge and pyo3 Python bindings for plugin authors.

/// Generated protocol messages.
pub mod proto {
    /// Messages compiled from `proto/plugin.proto`.
    pub mod plugin {
        #![allow(missing_docs, clippy::all, clippy::pedantic, clippy::nursery)]
        include!(concat!(env!("OUT_DIR"), "/foldit.plugin.rs"));
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub mod abi;

pub mod decode;
pub mod error;
pub mod export;
pub mod plugin;
pub mod protocol;

#[cfg(feature = "python")]
pub mod python;

pub use error::{PluginError, Result};
pub use plugin::{AssemblyPayload, Plugin};
pub use protocol::{DispatchContext, ParamValue, PollOutcome, ResidueRef};

#[cfg(feature = "python")]
use pyo3::prelude::*;

/// Python module entry point. Registers the receive-direction read types
/// (`DispatchContext`, `ResidueRef`) and the return-direction constructible
/// types (`PollOutcome` and the `ScoreReport` family) a plugin builds.
#[cfg(feature = "python")]
#[pymodule(name = "foldit_plugin_sdk", gil_used = true)]
fn foldit_plugin_sdk(_py: Python, m: &Bound<PyModule>) -> PyResult<()> {
    m.add_class::<ResidueRef>()?;
    m.add_class::<DispatchContext>()?;
    m.add_class::<python::PyPollOutcome>()?;
    m.add_class::<python::PyScoreReport>()?;
    m.add_class::<python::PyResidueTermScores>()?;
    m.add_class::<python::PyBonusContribution>()?;
    Ok(())
}
