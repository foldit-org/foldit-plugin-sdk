"""Logging configuration for foldit-runner Python modules.

This module provides a consistent logging interface across all model plugins.
Import and use get_logger() to get a logger for your module.

Usage:
    from logging_config import get_logger
    logger = get_logger(__name__)

    logger.info("Loading model...")
    logger.debug("Debug details: %s", details)
    logger.warning("Something unexpected")
    logger.error("Operation failed: %s", error)
"""

import logging
import sys

# Default log level - can be overridden via environment variable
_DEFAULT_LEVEL = logging.INFO

# Track if we've configured the root handler
_configured = False


def configure_logging(level=None):
    """Configure the root logger with a stderr handler.

    When ``FOLDIT_PLUGIN_LOG_PATH`` is set in the env (orchestrator sets
    it per spawn; see ``docs/PLUGIN_PROTOCOL.md`` §Logging), also
    attach a ``FileHandler`` at that path so plugin output lands in a
    forensic per-spawn file. The stderr handler stays attached so dev
    runs keep their console output.

    Args:
        level: Logging level (default: INFO, or DEBUG if FOLDIT_ML_DEBUG env var is set)
    """
    global _configured
    if _configured:
        return

    import os

    if level is None:
        if os.environ.get("FOLDIT_ML_DEBUG"):
            level = logging.DEBUG
        else:
            level = _DEFAULT_LEVEL

    root_logger = logging.getLogger("foldit_runner")
    root_logger.setLevel(level)

    # Stderr handler; dev console output, unchanged.
    stderr_handler = logging.StreamHandler(sys.stderr)
    stderr_handler.setLevel(level)
    stderr_handler.setFormatter(
        logging.Formatter(fmt="[%(name)s] %(message)s", datefmt="%H:%M:%S")
    )
    root_logger.addHandler(stderr_handler)

    # File handler keyed off the orchestrator-provided per-spawn path.
    log_path = os.environ.get("FOLDIT_PLUGIN_LOG_PATH")
    if log_path:
        try:
            file_handler = logging.FileHandler(log_path)
            file_handler.setLevel(level)
            file_handler.setFormatter(
                logging.Formatter(
                    fmt="%(asctime)s %(levelname)s %(name)s: %(message)s"
                )
            )
            root_logger.addHandler(file_handler)
        except OSError as e:
            # Don't fail plugin startup if the log file can't be opened;
            # stderr handler still covers dev visibility.
            sys.stderr.write(
                f"[foldit_plugin_sdk] could not open FOLDIT_PLUGIN_LOG_PATH={log_path}: {e}\n"
            )

    _configured = True


def get_logger(name):
    """Get a logger for the given module name.

    Args:
        name: Module name (typically __name__)

    Returns:
        Logger instance configured for foldit-runner
    """
    configure_logging()

    # Strip 'model_plugins.' prefix for cleaner output
    if name.startswith("model_plugins."):
        name = name[len("model_plugins.") :]
    elif name == "__main__":
        name = "main"

    return logging.getLogger(f"foldit_runner.{name}")
