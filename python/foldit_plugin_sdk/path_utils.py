"""Path and module utilities for foldit-runner plugins.

This module provides utilities for managing sys.path and working directories
when loading models that have conflicting module names or require specific
working directories.

Usage:
    from path_utils import working_directory, clear_modules, temporary_sys_path

    with working_directory("/path/to/model"):
        model = load_model()

    clear_modules(["simplefold", "utils"])  # Clear cached modules

    with temporary_sys_path(["/path/to/add"]):
        from some_module import something
"""

import os
import sys
from contextlib import contextmanager

from .logging_config import get_logger

logger = get_logger(__name__)


@contextmanager
def working_directory(path):
    """Context manager for temporarily changing the working directory.

    Args:
        path: Directory to change to

    Yields:
        None

    Example:
        with working_directory("/path/to/model"):
            model = load_model()  # load_model expects to be in /path/to/model
    """
    original_cwd = os.getcwd()
    try:
        os.chdir(path)
        logger.debug("Changed working directory to %s", path)
        yield
    finally:
        os.chdir(original_cwd)
        logger.debug("Restored working directory to %s", original_cwd)


@contextmanager
def temporary_sys_path(paths_to_add, prepend=True):
    """Context manager for temporarily modifying sys.path.

    Args:
        paths_to_add: List of paths to add to sys.path
        prepend: If True, add paths at the beginning; if False, append

    Yields:
        None

    Example:
        with temporary_sys_path(["/path/to/module"]):
            from module import something
    """
    original_path = sys.path.copy()
    try:
        if prepend:
            for path in reversed(paths_to_add):
                if path not in sys.path:
                    sys.path.insert(0, path)
        else:
            for path in paths_to_add:
                if path not in sys.path:
                    sys.path.append(path)
        logger.debug("Added paths to sys.path: %s", paths_to_add)
        yield
    finally:
        sys.path[:] = original_path
        logger.debug("Restored original sys.path")


def clear_modules(prefixes):
    """Clear modules from sys.modules matching given prefixes.

    This is useful when you need to re-import a module with different
    configuration, or when there are module name conflicts.

    Args:
        prefixes: List of module name prefixes to clear (e.g., ["utils", "simplefold"])

    Returns:
        List of module names that were cleared

    Example:
        clear_modules(["utils", "simplefold"])
        # Now 'utils' and 'simplefold.*' can be re-imported fresh
    """
    to_remove = []
    for key in list(sys.modules.keys()):
        for prefix in prefixes:
            if key == prefix or key.startswith(f"{prefix}."):
                to_remove.append(key)
                break

    for key in to_remove:
        del sys.modules[key]

    if to_remove:
        logger.debug(
            "Cleared %d modules from sys.modules: %s", len(to_remove), to_remove
        )

    return to_remove


def deprioritize_paths(substring):
    """Move paths containing a substring to the end of sys.path.

    This is useful when a dependency has a module with a common name
    (like 'utils') that conflicts with another package's module.

    Args:
        substring: Substring to match in path entries (case-insensitive)

    Returns:
        Number of paths that were moved

    Example:
        # Move our 'python/' paths to the end so simplefold's 'utils' is found first
        deprioritize_paths("python")
    """
    substring_lower = substring.lower()
    matching = []
    non_matching = []

    for path in sys.path:
        if substring_lower in path.lower():
            matching.append(path)
        else:
            non_matching.append(path)

    if matching:
        sys.path[:] = non_matching + matching
        logger.debug(
            "Moved %d paths containing '%s' to end of sys.path",
            len(matching),
            substring,
        )

    return len(matching)
