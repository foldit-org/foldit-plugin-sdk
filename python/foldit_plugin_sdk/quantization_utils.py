"""Quantization utilities for ML models.

This module provides backend-agnostic quantization helpers for MLX and PyTorch models.
Supports 4-bit and 8-bit quantization.

Usage:
    from quantization_utils import quantize_model, quantize_torch_model, quantize_mlx_model

    # Auto-detect backend and quantize
    quantize_model(model, bits=8, backend="mlx")

    # Or use backend-specific functions
    quantize_torch_model(model, bits=8)
    quantize_mlx_model(model, bits=8, can_quantize_predicate=my_predicate)
"""

import torch
from .logging_config import get_logger

logger = get_logger(__name__)


def default_can_quantize_predicate(path, module):
    """Default predicate for determining if a layer can be quantized.

    Only quantizes Linear layers where the weight dimension is divisible by 64
    (required for efficient quantization).

    Args:
        path: Module path in the model hierarchy
        module: The module to check

    Returns:
        True if the module can be quantized, False otherwise
    """
    if not isinstance(module, torch.nn.Linear):
        return False
    if hasattr(module, "weight"):
        weight_shape = module.weight.shape
        if len(weight_shape) >= 2 and weight_shape[-1] % 64 != 0:
            return False
    return True


def quantize_torch_model(model, bits=8, model_name="model"):
    """Apply quantization to a PyTorch model using optimum-quanto.

    Args:
        model: PyTorch model to quantize
        bits: Quantization bits (4 or 8)
        model_name: Name for logging purposes

    Returns:
        True if quantization succeeded, False otherwise
    """
    try:
        from optimum.quanto import quantize, freeze, qint4, qint8

        weights = qint4 if bits == 4 else qint8
        quantize(model, weights=weights)
        freeze(model)
        logger.info(
            "%d-bit quantization applied to %s with optimum-quanto", bits, model_name
        )
        return True
    except ImportError:
        logger.warning(
            "optimum-quanto not available, skipping quantization for %s", model_name
        )
        return False
    except Exception as e:
        logger.warning(
            "PyTorch quantization failed for %s: %s, using full precision",
            model_name,
            e,
        )
        return False


def quantize_mlx_model(model, bits=8, model_name="model", can_quantize_predicate=None):
    """Apply quantization to an MLX model.

    Args:
        model: MLX model to quantize
        bits: Quantization bits (4 or 8)
        model_name: Name for logging purposes
        can_quantize_predicate: Optional function(path, module) -> bool to filter layers

    Returns:
        True if quantization succeeded, False otherwise
    """
    if can_quantize_predicate is None:
        can_quantize_predicate = default_can_quantize_predicate

    try:
        from mlx.nn import quantize as mlx_quantize

        mlx_quantize(
            model,
            group_size=64,
            bits=bits,
            class_predicate=can_quantize_predicate,
        )
        logger.info(
            "%d-bit quantization applied to compatible layers of %s", bits, model_name
        )
        return True
    except ImportError:
        logger.warning("MLX not available, skipping quantization for %s", model_name)
        return False
    except Exception as e:
        logger.warning(
            "MLX quantization failed for %s: %s, using full precision", model_name, e
        )
        return False


def quantize_model(
    model, bits, backend, model_name="model", can_quantize_predicate=None
):
    """Apply quantization to a model using the appropriate backend.

    Args:
        model: Model to quantize (PyTorch or MLX)
        bits: Quantization bits (4 or 8)
        backend: "torch" or "mlx"
        model_name: Name for logging purposes
        can_quantize_predicate: For MLX, optional function(path, module) -> bool

    Returns:
        True if quantization succeeded, False otherwise
    """
    logger.info("Applying %d-bit quantization to %s...", bits, model_name)

    if backend == "torch":
        return quantize_torch_model(model, bits=bits, model_name=model_name)
    elif backend == "mlx":
        return quantize_mlx_model(
            model,
            bits=bits,
            model_name=model_name,
            can_quantize_predicate=can_quantize_predicate,
        )
    else:
        logger.warning("Unknown backend %s, skipping quantization", backend)
        return False
