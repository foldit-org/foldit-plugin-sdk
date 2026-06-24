//! pyo3 bindings for the return-direction protocol types a plugin builds.
//!
//! `DispatchContext` and `ResidueRef` (receive direction, read-only) carry
//! their pyclass annotations inline in [`crate::protocol`]. The constructible
//! types live here: a plugin builds a [`PyScoreReport`] from parallel lists and
//! returns a [`PollOutcome`] via its `pending` / `checkpoint` / `cancelled` /
//! `final_` / `error` factories.
//!
//! `PollOutcome` is exposed as an opaque handle plus `#[staticmethod]`
//! factories rather than a native data-carrying enum pyclass: that is the
//! proven in-tree idiom (molex's `Variant`, edit views) and it keeps each
//! variant's field set explicit at the factory signature.

use std::collections::HashMap;

use pyo3::prelude::*;

use crate::proto::plugin::{
    self as proto, ResidueTermScores as ProtoResidueTermScores, ScoreReport,
};
use crate::protocol::PollOutcome as RustPollOutcome;

/// One labeled puzzle-objective bonus contribution. Mirrors
/// `proto::plugin::BonusContribution`.
#[pyclass(
    name = "BonusContribution",
    module = "foldit_plugin_sdk",
    from_py_object
)]
#[derive(Clone)]
pub struct PyBonusContribution {
    inner: proto::BonusContribution,
}

#[pymethods]
impl PyBonusContribution {
    /// Build a bonus contribution: a filter `kind` name plus its raw
    /// rosetta-energy `value`.
    #[new]
    #[must_use]
    pub fn new(kind: String, value: f32) -> Self {
        Self {
            inner: proto::BonusContribution { kind, value },
        }
    }

    /// Filter's internal name (e.g. `"DisulfideCountScore"`).
    #[must_use]
    #[getter]
    pub fn kind(&self) -> String {
        self.inner.kind.clone()
    }

    /// Raw rosetta-energy contribution.
    #[must_use]
    #[getter]
    pub fn value(&self) -> f32 {
        self.inner.value
    }
}

/// Per-residue raw term scores for one residue. Mirrors
/// `proto::plugin::ResidueTermScores`; `terms` aligns to
/// [`PyScoreReport::term_names`].
#[pyclass(
    name = "ResidueTermScores",
    module = "foldit_plugin_sdk",
    from_py_object
)]
#[derive(Clone)]
pub struct PyResidueTermScores {
    inner: ProtoResidueTermScores,
}

#[pymethods]
impl PyResidueTermScores {
    /// Build a per-residue score row from the residue's `entity_id` /
    /// `residue_index` and its raw unweighted `terms` (aligned to
    /// `term_names`).
    #[new]
    #[must_use]
    pub fn new(entity_id: u64, residue_index: u32, terms: Vec<f32>) -> Self {
        Self {
            inner: ProtoResidueTermScores {
                residue: Some(proto::ResidueRef {
                    entity_id,
                    residue_index,
                }),
                terms,
            },
        }
    }

    /// Raw entity id the row belongs to, or `None` if unset.
    #[must_use]
    #[getter]
    pub fn entity_id(&self) -> Option<u64> {
        self.inner.residue.as_ref().map(|r| r.entity_id)
    }

    /// 0-indexed residue position, or `None` if unset.
    #[must_use]
    #[getter]
    pub fn residue_index(&self) -> Option<u32> {
        self.inner.residue.as_ref().map(|r| r.residue_index)
    }

    /// Raw unweighted per-term scores, aligned to `term_names`.
    #[must_use]
    #[getter]
    pub fn terms(&self) -> Vec<f32> {
        self.inner.terms.clone()
    }
}

/// Raw unweighted score breakdown a plugin attaches to a poll outcome.
///
/// Mirrors `proto::plugin::ScoreReport`: the host owns weighting, so the
/// arrays are raw and align to `term_names` (same order, same length).
#[pyclass(name = "ScoreReport", module = "foldit_plugin_sdk", from_py_object)]
#[derive(Clone)]
pub struct PyScoreReport {
    pub(crate) inner: ScoreReport,
}

#[pymethods]
impl PyScoreReport {
    /// Build a score report. `whole_pose_terms` aligns to `term_names`;
    /// each entry in `per_residue_terms` carries a parallel `terms` array.
    /// `bonus_breakdown` defaults to empty.
    #[new]
    #[pyo3(signature = (term_names, whole_pose_terms, per_residue_terms, bonus_breakdown = Vec::new()))]
    #[must_use]
    pub fn new(
        term_names: Vec<String>,
        whole_pose_terms: Vec<f32>,
        per_residue_terms: Vec<PyResidueTermScores>,
        bonus_breakdown: Vec<PyBonusContribution>,
    ) -> Self {
        Self {
            inner: ScoreReport {
                term_names,
                whole_pose_terms,
                per_residue_terms: per_residue_terms.into_iter().map(|r| r.inner).collect(),
                bonus_breakdown: bonus_breakdown.into_iter().map(|b| b.inner).collect(),
            },
        }
    }

    /// Ordered raw term labels the score arrays align to.
    #[must_use]
    #[getter]
    pub fn term_names(&self) -> Vec<String> {
        self.inner.term_names.clone()
    }

    /// Whole-pose raw term totals, aligned to `term_names`.
    #[must_use]
    #[getter]
    pub fn whole_pose_terms(&self) -> Vec<f32> {
        self.inner.whole_pose_terms.clone()
    }

    /// Per-residue raw term rows.
    #[must_use]
    #[getter]
    pub fn per_residue_terms(&self) -> Vec<PyResidueTermScores> {
        self.inner
            .per_residue_terms
            .iter()
            .cloned()
            .map(|inner| PyResidueTermScores { inner })
            .collect()
    }

    /// Labeled puzzle-objective bonus contributions.
    #[must_use]
    #[getter]
    pub fn bonus_breakdown(&self) -> Vec<PyBonusContribution> {
        self.inner
            .bonus_breakdown
            .iter()
            .cloned()
            .map(|inner| PyBonusContribution { inner })
            .collect()
    }
}

/// Python-constructible poll outcome. Build one of the five terminals/states
/// via the `#[staticmethod]` factories; the host reads the variant back via
/// `kind` plus the per-field getters.
#[pyclass(name = "PollOutcome", module = "foldit_plugin_sdk", from_py_object)]
#[derive(Clone)]
pub struct PyPollOutcome {
    inner: RustPollOutcome,
}

#[pymethods]
impl PyPollOutcome {
    /// Stream still running; `latest_assembly` is a discardable preview frame.
    #[staticmethod]
    #[pyo3(signature = (latest_assembly = None, progress = None, stage = None, score = None))]
    #[must_use]
    pub fn pending(
        latest_assembly: Option<Vec<u8>>,
        progress: Option<f32>,
        stage: Option<String>,
        score: Option<PyScoreReport>,
    ) -> Self {
        Self {
            inner: RustPollOutcome::Pending {
                latest_assembly,
                progress,
                stage,
                score: score.map(|s| s.inner),
            },
        }
    }

    /// Accepted intermediate the host commits while the stream keeps running.
    #[staticmethod]
    #[pyo3(signature = (latest_assembly = None, progress = None, stage = None, score = None))]
    #[must_use]
    pub fn checkpoint(
        latest_assembly: Option<Vec<u8>>,
        progress: Option<f32>,
        stage: Option<String>,
        score: Option<PyScoreReport>,
    ) -> Self {
        Self {
            inner: RustPollOutcome::Checkpoint {
                latest_assembly,
                progress,
                stage,
                score: score.map(|s| s.inner),
            },
        }
    }

    /// Stream stopped at host request, returning a usable working pose.
    #[staticmethod]
    #[pyo3(signature = (assembly, score = None))]
    #[must_use]
    pub fn cancelled(assembly: Vec<u8>, score: Option<PyScoreReport>) -> Self {
        Self {
            inner: RustPollOutcome::Cancelled {
                assembly,
                score: score.map(|s| s.inner),
            },
        }
    }

    /// Stream finished successfully; `assembly` is the definitive output.
    #[staticmethod]
    #[pyo3(signature = (assembly, score = None))]
    #[must_use]
    pub fn final_(assembly: Vec<u8>, score: Option<PyScoreReport>) -> Self {
        Self {
            inner: RustPollOutcome::Final {
                assembly,
                score: score.map(|s| s.inner),
            },
        }
    }

    /// Op-level failure.
    #[staticmethod]
    #[must_use]
    pub fn error(code: String, message: String, details: HashMap<String, String>) -> Self {
        Self {
            inner: RustPollOutcome::Error {
                code,
                message,
                details,
            },
        }
    }

    /// Variant discriminant: one of `"pending"`, `"checkpoint"`,
    /// `"cancelled"`, `"final"`, `"error"`.
    #[must_use]
    #[getter]
    pub fn kind(&self) -> &'static str {
        match self.inner {
            RustPollOutcome::Pending { .. } => "pending",
            RustPollOutcome::Checkpoint { .. } => "checkpoint",
            RustPollOutcome::Cancelled { .. } => "cancelled",
            RustPollOutcome::Final { .. } => "final",
            RustPollOutcome::Error { .. } => "error",
        }
    }

    /// Working assembly bytes, regardless of variant. `Pending` / `Checkpoint`
    /// may omit it (`None`); `Cancelled` / `Final` always carry it; `Error`
    /// has none.
    #[must_use]
    #[getter]
    pub fn assembly(&self) -> Option<Vec<u8>> {
        match &self.inner {
            RustPollOutcome::Pending {
                latest_assembly, ..
            }
            | RustPollOutcome::Checkpoint {
                latest_assembly, ..
            } => latest_assembly.clone(),
            RustPollOutcome::Cancelled { assembly, .. }
            | RustPollOutcome::Final { assembly, .. } => Some(assembly.clone()),
            RustPollOutcome::Error { .. } => None,
        }
    }

    /// Progress fraction in `[0.0, 1.0]` for `Pending` / `Checkpoint`; `None`
    /// otherwise or when the plugin does not track it.
    #[must_use]
    #[getter]
    pub fn progress(&self) -> Option<f32> {
        match self.inner {
            RustPollOutcome::Pending { progress, .. }
            | RustPollOutcome::Checkpoint { progress, .. } => progress,
            _ => None,
        }
    }

    /// Human-readable stage label for `Pending` / `Checkpoint`; `None`
    /// otherwise or when the plugin does not provide one.
    #[must_use]
    #[getter]
    pub fn stage(&self) -> Option<String> {
        match &self.inner {
            RustPollOutcome::Pending { stage, .. } | RustPollOutcome::Checkpoint { stage, .. } => {
                stage.clone()
            }
            _ => None,
        }
    }

    /// Warm score of the outcome's assembly, if the plugin scored it. `None`
    /// for `Error`.
    #[must_use]
    #[getter]
    pub fn score(&self) -> Option<PyScoreReport> {
        let report = match &self.inner {
            RustPollOutcome::Pending { score, .. }
            | RustPollOutcome::Checkpoint { score, .. }
            | RustPollOutcome::Cancelled { score, .. }
            | RustPollOutcome::Final { score, .. } => score.clone(),
            RustPollOutcome::Error { .. } => None,
        };
        report.map(|inner| PyScoreReport { inner })
    }

    /// Machine-readable error code for the `Error` variant; `None` otherwise.
    #[must_use]
    #[getter]
    pub fn error_code(&self) -> Option<String> {
        match &self.inner {
            RustPollOutcome::Error { code, .. } => Some(code.clone()),
            _ => None,
        }
    }

    /// Human-readable error message for the `Error` variant; `None` otherwise.
    #[must_use]
    #[getter]
    pub fn error_message(&self) -> Option<String> {
        match &self.inner {
            RustPollOutcome::Error { message, .. } => Some(message.clone()),
            _ => None,
        }
    }

    /// Structured error detail map for the `Error` variant; empty otherwise.
    #[must_use]
    #[getter]
    pub fn error_details(&self) -> HashMap<String, String> {
        match &self.inner {
            RustPollOutcome::Error { details, .. } => details.clone(),
            _ => HashMap::new(),
        }
    }
}

impl PyPollOutcome {
    /// Consume the handle into its native `PollOutcome` (host-side path).
    #[must_use]
    pub fn into_inner(self) -> RustPollOutcome {
        self.inner
    }
}
