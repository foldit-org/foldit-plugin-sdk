"""Checkpoint discovery utilities for foldit-runner plugins.

These are pure *detection* helpers: they locate a weight file / model
directory and raise a clean ``FileNotFoundError`` when it is absent. They
do NOT download anything; fetching is the plugin's own ``download_weights``
op (see :mod:`foldit_plugin_sdk.weights`). The error a plugin surfaces
when a load is attempted before the weights exist points the user at that
op rather than at any out-of-band CLI command.

Usage:
    from foldit_plugin_sdk.checkpoint_utils import (
        find_checkpoint, ensure_checkpoint_exists,
    )

    # For foundry models (glob pattern)
    checkpoint = find_checkpoint(
        cache_dir, pattern="rf3_*.ckpt", model_name="RoseTTAFold3"
    )

    # For direct path models
    ensure_checkpoint_exists(
        path="<plugin_dir>/assets/weights/esm2/esm2_t36_3B_UR50D.pt",
        model_name="ESM2",
    )
"""

import os
from pathlib import Path
from typing import List, Optional

from .logging_config import get_logger

logger = get_logger(__name__)

# Appended to every "not found" message: weights are fetched in-process by
# the plugin's own download op, not by any external command.
_DOWNLOAD_HINT = "Run this plugin's 'download_weights' op to fetch it."


def find_checkpoint(
    cache_dir: str,
    pattern: str,
    model_name: str,
    search_dirs: Optional[List[str]] = None,
) -> str:
    """Find a model checkpoint matching a glob pattern.

    Searches for checkpoint files matching the given pattern in one or more
    subdirectories of the cache directory.

    Args:
        cache_dir: Base cache directory (e.g., <plugin_dir>/assets/weights)
        pattern: Glob pattern to match (e.g., "rf3_*.ckpt")
        model_name: Human-readable model name for error messages
        search_dirs: List of subdirectories to search (default: ["rc_foundry"])

    Returns:
        Path to the first matching checkpoint file

    Raises:
        FileNotFoundError: If no matching checkpoint is found

    Example:
        checkpoint = find_checkpoint(
            cache_dir="<plugin_dir>/assets/weights",
            pattern="rf3_*.ckpt",
            model_name="RoseTTAFold3"
        )
    """
    if search_dirs is None:
        search_dirs = ["rc_foundry"]

    searched_paths = []

    for subdir in search_dirs:
        search_path = Path(cache_dir) / subdir
        searched_paths.append(str(search_path))

        if search_path.exists():
            matches = list(search_path.glob(pattern))
            if matches:
                checkpoint_path = str(matches[0])
                logger.info("Found %s checkpoint: %s", model_name, checkpoint_path)
                return checkpoint_path

    # No checkpoint found - raise helpful error
    raise FileNotFoundError(
        f"{model_name} checkpoint not found matching pattern '{pattern}'\n"
        f"Searched in: {', '.join(searched_paths)}\n"
        f"{_DOWNLOAD_HINT}"
    )


def ensure_checkpoint_exists(path: str, model_name: str) -> str:
    """Ensure a checkpoint file exists at the given path.

    Args:
        path: Expected path to the checkpoint file
        model_name: Human-readable model name for error messages

    Returns:
        The path (unchanged) if file exists

    Raises:
        FileNotFoundError: If the checkpoint file doesn't exist

    Example:
        path = ensure_checkpoint_exists(
            path="<plugin_dir>/assets/weights/esm2/esm2_t36_3B_UR50D.pt",
            model_name="ESM2",
        )
    """
    expanded_path = os.path.expanduser(path)

    if os.path.exists(expanded_path):
        logger.debug("Checkpoint exists: %s", expanded_path)
        return expanded_path

    raise FileNotFoundError(
        f"{model_name} checkpoint not found at {expanded_path}\n{_DOWNLOAD_HINT}"
    )


def ensure_directory_exists(path: str, model_name: str) -> str:
    """Ensure a model directory exists (for HuggingFace-style models).

    Args:
        path: Expected path to the model directory
        model_name: Human-readable model name for error messages

    Returns:
        The path (unchanged) if directory exists

    Raises:
        FileNotFoundError: If the directory doesn't exist

    Example:
        path = ensure_directory_exists(
            path="<plugin_dir>/assets/weights/my-hf-model",
            model_name="MyHFModel",
        )
    """
    expanded_path = os.path.expanduser(path)

    if os.path.isdir(expanded_path):
        logger.debug("Model directory exists: %s", expanded_path)
        return expanded_path

    raise FileNotFoundError(
        f"{model_name} model not found at {expanded_path}\n{_DOWNLOAD_HINT}"
    )
