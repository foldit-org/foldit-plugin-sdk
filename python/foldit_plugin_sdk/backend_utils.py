"""Backend selection utilities for foldit-runner plugins.

This module provides utilities for selecting the appropriate compute backend
(MLX for Apple Silicon, PyTorch for other platforms).

Usage:
    from backend_utils import get_backend

    backend = get_backend()  # Returns "mlx" on macOS, "torch" otherwise
"""

import platform


def get_backend() -> str:
    """Get the appropriate backend for the current platform.

    Returns "mlx" on macOS (Apple Silicon optimized), "torch" on other platforms.

    Returns:
        str: Backend identifier ("mlx" or "torch")

    Example:
        backend = get_backend()
        model = ModelWrapper(backend=backend)
    """
    return "mlx" if platform.system() == "Darwin" else "torch"


def is_mlx_available() -> bool:
    """Check if MLX backend is available.

    Returns:
        bool: True if running on macOS (where MLX is available)
    """
    return platform.system() == "Darwin"


def is_torch_cuda_available() -> bool:
    """Check if PyTorch CUDA is available.

    Returns:
        bool: True if CUDA-capable GPU is available
    """
    try:
        import torch

        return torch.cuda.is_available()
    except ImportError:
        return False


def log_backend_info(logger, backend: str, device=None) -> None:
    """Log information about the selected backend.

    Args:
        logger: Logger instance to use for output
        backend: Backend identifier ("mlx" or "torch")
        device: Optional device info to log for torch backend
    """
    if backend == "mlx":
        logger.info("Backend: MLX (Apple Silicon GPU)")
        logger.info(
            "Note: First load converts PyTorch->MLX (~30-60s), subsequent loads faster"
        )
    else:
        logger.info("Backend: PyTorch")
        if device is not None:
            logger.info("Device: %s", device)
