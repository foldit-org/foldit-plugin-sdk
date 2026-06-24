"""Device detection utilities for foldit-runner.

Provides a consistent interface for detecting and selecting compute devices
(CPU, CUDA GPU, or Apple MPS) across all model plugins.
"""

import torch


def get_best_device(prefer_cpu=False, allow_mps=True):
    """Get the best available compute device.

    Args:
        prefer_cpu: If True, always return CPU regardless of GPU availability
        allow_mps: If True, allow MPS (Apple Silicon GPU) to be selected

    Returns:
        torch.device: The selected device (cuda, mps, or cpu)
    """
    if prefer_cpu:
        return torch.device("cpu")

    if torch.cuda.is_available():
        return torch.device("cuda")

    if allow_mps and torch.backends.mps.is_available():
        return torch.device("mps")

    return torch.device("cpu")


def get_device_info():
    """Get information about available compute devices.

    Returns:
        dict: Device availability information with keys:
            - cuda_available: bool
            - mps_available: bool
            - best_device: str (device name)
    """
    cuda_available = torch.cuda.is_available()
    mps_available = torch.backends.mps.is_available()

    if cuda_available:
        best = "cuda"
    elif mps_available:
        best = "mps"
    else:
        best = "cpu"

    return {
        "cuda_available": cuda_available,
        "mps_available": mps_available,
        "best_device": best,
    }


def log_device_info(logger):
    """Log information about available compute devices.

    Args:
        logger: Logger instance to use for output
    """
    info = get_device_info()

    if info["cuda_available"]:
        logger.info("CUDA GPU available")
    elif info["mps_available"]:
        logger.info("MPS (Apple GPU) available")
    else:
        logger.info("No GPU available - will use CPU")
