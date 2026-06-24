"""Multiprocessing configuration utilities for foldit-runner plugins.

This module handles the complexities of running Python multiprocessing
when embedded in a Rust binary (PyO3) or when PYTHONHOME is set.

Usage:
    from multiprocessing_utils import configure_multiprocessing
    configure_multiprocessing()  # Call early, before importing multiprocessing-dependent code
"""

import os
import sys
import multiprocessing

from .logging_config import get_logger

logger = get_logger(__name__)

# Track whether we've already configured multiprocessing
_configured = False


def find_python_executable():
    """Find a suitable Python executable for multiprocessing.

    When running under PyO3 (embedded in Rust binary) or with PYTHONHOME set,
    sys.executable may not point to a valid Python interpreter. This function
    searches for a working Python executable.

    Returns:
        Path to Python executable, or None if current sys.executable should be kept
    """
    # Check if we're running inside a Rust worker binary
    if sys.executable.endswith("foldit-worker"):
        env_dir = os.path.dirname(os.path.dirname(sys.executable))

        possible_paths = [
            # 1. Python in same directory as worker (bundle layout)
            os.path.join(os.path.dirname(sys.executable), "python"),
            os.path.join(os.path.dirname(sys.executable), "python3"),
            # 2. Pixi environments (development layout)
            os.path.join(env_dir, ".pixi", "envs", "foundry", "bin", "python"),
            os.path.join(env_dir, ".pixi", "envs", "foundry-cpu", "bin", "python"),
        ]

        # 3. sys.base_executable (Python 3.10+)
        if hasattr(sys, "base_executable") and sys.base_executable:
            possible_paths.append(sys.base_executable)

        # 4. Fallback to python3 in PATH
        possible_paths.append("python3")

        for path in possible_paths:
            if os.path.exists(path):
                return path

        logger.warning(
            "Could not find Python interpreter, keeping sys.executable as %s",
            sys.executable,
        )
        return None

    # Check if PYTHONHOME is set (common in bundled environments)
    if "PYTHONHOME" in os.environ:
        python_exe = os.path.join(os.environ["PYTHONHOME"], "bin", "python")
        if os.path.exists(python_exe):
            return python_exe
        else:
            logger.warning("Python executable not found at %s", python_exe)
            return None

    # sys.executable is probably fine
    logger.debug("PYTHONHOME not set, sys.executable is %s", sys.executable)
    return None


def configure_multiprocessing():
    """Configure multiprocessing for use in foldit-runner plugins.

    This function:
    1. Fixes sys.executable if running under PyO3 or PYTHONHOME
    2. Sets the multiprocessing start method to 'spawn' (required for CUDA/MPS)
    3. Sets multiprocessing.set_executable if available

    Should be called early in plugin initialization, before any code that
    uses multiprocessing.

    This function is idempotent - safe to call multiple times.
    """
    global _configured
    if _configured:
        return

    # Fix sys.executable if needed
    python_path = find_python_executable()
    if python_path:
        sys.executable = python_path
        logger.info("Set sys.executable to %s", python_path)

    # Set start method to 'spawn' to avoid fork issues with CUDA/MPS
    # 'spawn' creates a fresh Python interpreter for each child process
    try:
        multiprocessing.set_start_method("spawn", force=True)
        logger.debug("multiprocessing start method set to 'spawn'")
    except RuntimeError:
        logger.debug(
            "multiprocessing start method already set: %s",
            multiprocessing.get_start_method(),
        )

    # Set the executable for multiprocessing explicitly
    # This prevents child processes from trying to execute the Rust worker binary
    if hasattr(multiprocessing, "set_executable"):
        multiprocessing.set_executable(sys.executable)
        logger.debug("multiprocessing executable set to %s", sys.executable)

    _configured = True
