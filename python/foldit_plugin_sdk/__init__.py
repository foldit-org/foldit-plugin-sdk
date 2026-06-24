"""foldit-plugin-sdk: protocol types and plugin-author utilities.

Plugins subclass :class:`PluginInterface` and use the protobuf bindings
under :mod:`foldit_plugin_sdk.proto` to register ops + queries with the
foldit orchestrator. The dispatch context, poll outcome, and score-report
types are native pyo3 classes compiled into this package's extension and
re-exported here. Shared ML utilities (checkpoint discovery, device
selection, etc.) ship as sibling submodules.

Plugin authors typically::

    from foldit_plugin_sdk import PluginInterface, DispatchContext, PollOutcome
    from foldit_plugin_sdk.proto import plugin_pb2
    from foldit_plugin_sdk import checkpoint_utils, device_utils
"""

from __future__ import annotations

# Native pyo3 classes compiled into the same-name extension module that
# maturin nests inside this package. The receive-direction read types
# (DispatchContext, ResidueRef) plus the return-direction constructible
# types (PollOutcome and the ScoreReport family) all live there.
from .foldit_plugin_sdk import (  # type: ignore[import-not-found]
    BonusContribution,
    DispatchContext,
    PollOutcome,
    ResidueRef,
    ResidueTermScores,
    ScoreReport,
)
from .plugin_interface import (
    PluginInterface,
    find_plugin_class,
    make_param_value,
)

__all__ = [
    "BonusContribution",
    "DispatchContext",
    "PollOutcome",
    "ResidueRef",
    "ResidueTermScores",
    "ScoreReport",
    "PluginInterface",
    "find_plugin_class",
    "make_param_value",
]

__version__ = "0.1.0"
