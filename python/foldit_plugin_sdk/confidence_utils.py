"""Confidence score utilities for foldit-runner plugins.

This module provides utilities for extracting and normalizing confidence scores
from model outputs. pLDDT (predicted Local Distance Difference Test) values
are commonly on a 0-100 scale, which we normalize to 0-1 for consistency.

Usage:
    from confidence_utils import plddt_to_confidence, extract_mean_plddt

    # Convert pLDDT array to confidence
    confidence = plddt_to_confidence(plddt_array)

    # Extract from model output dict
    confidence = extract_mean_plddt(output_dict)
"""

from typing import Union, Any, Optional
import numpy as np

from .logging_config import get_logger

logger = get_logger(__name__)


def plddt_to_confidence(
    plddt: Union[float, list, np.ndarray, Any],
    scale: float = 100.0,
) -> float:
    """Convert pLDDT values to a confidence score in [0, 1].

    Handles various input types:
    - Scalar float/int
    - Python list
    - NumPy array
    - Tensor-like objects with .mean() method (PyTorch, MLX)

    Args:
        plddt: pLDDT values (0-100 scale by default)
        scale: Scale factor to divide by (default: 100.0 for pLDDT)

    Returns:
        float: Mean confidence score in [0, 1] range

    Example:
        # From tensor with .mean()
        confidence = plddt_to_confidence(model_output["plddt"])

        # From list
        confidence = plddt_to_confidence([85.2, 90.1, 78.5])

        # From scalar
        confidence = plddt_to_confidence(87.5)
    """
    if plddt is None:
        return 0.0

    # Handle tensor-like objects with .mean() method (PyTorch, MLX, etc.)
    if hasattr(plddt, "mean"):
        mean_plddt = float(plddt.mean())
    # Handle numpy arrays
    elif isinstance(plddt, np.ndarray):
        mean_plddt = float(np.mean(plddt))
    # Handle lists/sequences
    elif isinstance(plddt, (list, tuple)) and len(plddt) > 0:
        mean_plddt = float(sum(plddt) / len(plddt))
    # Handle scalar
    elif isinstance(plddt, (int, float)):
        mean_plddt = float(plddt)
    else:
        logger.warning("Unknown pLDDT type %s, returning 0.0", type(plddt))
        return 0.0

    # Normalize to [0, 1]
    confidence = mean_plddt / scale

    # Ensure result is a Python float (not numpy scalar)
    return float(confidence)


def extract_mean_plddt(
    output: dict,
    mean_key: str = "mean_plddt",
    per_residue_key: str = "plddt",
    default: float = 0.0,
) -> float:
    """Extract mean pLDDT confidence from a model output dictionary.

    Tries to extract from mean_plddt first, falls back to computing mean
    from per-residue pLDDT values.

    Args:
        output: Dictionary from model output
        mean_key: Key for pre-computed mean pLDDT (default: "mean_plddt")
        per_residue_key: Key for per-residue pLDDT array (default: "plddt")
        default: Default value if neither key exists (default: 0.0)

    Returns:
        float: Mean confidence score in [0, 1] range

    Example:
        output = model(**inputs)
        confidence = extract_mean_plddt(output)
    """
    if mean_key in output:
        return plddt_to_confidence(output[mean_key])
    elif per_residue_key in output:
        return plddt_to_confidence(output[per_residue_key])
    else:
        logger.debug(
            "No pLDDT found in output (tried '%s' and '%s'), returning default %.1f",
            mean_key,
            per_residue_key,
            default,
        )
        return default
