"""Cache file management utilities for foldit-runner plugins.

This module provides utilities for managing cached model files,
including symlink creation with copy fallback for cross-filesystem support.

Usage:
    from cache_utils import link_or_copy, setup_cache_files

    # Single file
    link_or_copy(source="/path/to/downloaded/file", dest="/path/to/expected/file")

    # Multiple files with mapping
    setup_cache_files(
        source_dir="/path/to/downloaded",
        dest_dir="/path/to/expected",
        file_mappings=[("ccd.pkl", "ccd.pkl"), ("model.ckpt", "model.ckpt")]
    )
"""

import os
import shutil
from pathlib import Path
from typing import List, Tuple, Union

from .logging_config import get_logger

logger = get_logger(__name__)


def weights_dir(plugin_dir: Union[str, "os.PathLike[str]"]) -> str:
    """Return ``<plugin_dir>/assets/weights`` for an explicit plugin dir.

    A plugin owns its weights the way the native rosetta plugin owns its
    database at ``<plugin_dir>/assets/database/``: they are plugin assets
    that live under the plugin's own directory, resolved relative to it.

    Use this from code that already knows the plugin directory (download
    scripts, tests). Plugin instances should call
    :func:`weights_dir_from_config` instead, since the host hands them
    their ``plugin_dir`` in the construction config.
    """
    return str(Path(plugin_dir) / "assets" / "weights")


def weights_dir_from_config(config: dict) -> str:
    """Resolve a plugin's weight root from its host-provided config.

    The orchestrator hands every plugin its ``plugin_dir`` in the config
    dict at construction (see ``PLUGIN_PROTOCOL.md`` §"Plugin
    self-configuration"). Weights live under ``<plugin_dir>/assets/weights/``.

    Plugins call this in ``__init__`` to resolve their cache root.

    Raises:
        RuntimeError: if ``config`` carries no ``plugin_dir`` (a host
            wiring bug; the python-host must forward it).
    """
    try:
        plugin_dir = config["plugin_dir"]
    except (KeyError, TypeError):
        raise RuntimeError(
            "plugin config missing 'plugin_dir'; the host must forward it "
            "(see PLUGIN_PROTOCOL.md §\"Plugin self-configuration\")"
        )
    return weights_dir(plugin_dir)


def link_or_copy(source: str, dest: str, skip_existing: bool = True) -> bool:
    """Create a symlink to source at dest, falling back to copy if symlink fails.

    This handles cross-filesystem scenarios where symlinks may not work
    (e.g., different mount points, Windows compatibility).

    Args:
        source: Path to the source file
        dest: Path where the link/copy should be created
        skip_existing: If True, skip if dest already exists (default: True)

    Returns:
        True if link/copy was created, False if skipped or source doesn't exist

    Example:
        link_or_copy(
            source="~/.cache/downloads/model.pt",
            dest="<plugin_dir>/assets/weights/model.pt"
        )
    """
    source = os.path.expanduser(source)
    dest = os.path.expanduser(dest)

    if not os.path.exists(source):
        logger.debug("Source file does not exist: %s", source)
        return False

    if skip_existing and os.path.exists(dest):
        logger.debug("Destination already exists, skipping: %s", dest)
        return False

    # Ensure destination directory exists
    dest_dir = os.path.dirname(dest)
    if dest_dir:
        os.makedirs(dest_dir, exist_ok=True)

    try:
        os.symlink(source, dest)
        logger.debug("Created symlink: %s -> %s", dest, source)
        return True
    except OSError as e:
        logger.debug("Symlink failed (%s), falling back to copy", e)
        shutil.copy2(source, dest)
        logger.debug("Copied file: %s -> %s", source, dest)
        return True


def setup_cache_files(
    source_dir: str,
    dest_dir: str,
    file_mappings: List[Tuple[str, str]],
    skip_existing: bool = True,
) -> int:
    """Set up multiple cache files by linking or copying from source to dest.

    Args:
        source_dir: Directory containing downloaded/source files
        dest_dir: Directory where files should be available
        file_mappings: List of (dest_name, source_name) tuples
        skip_existing: If True, skip files that already exist at dest

    Returns:
        Number of files that were linked/copied

    Example:
        setup_cache_files(
            source_dir="<plugin_dir>/assets/weights/simplefold_cache",
            dest_dir="<plugin_dir>/assets/weights/simplefold_output/cache",
            file_mappings=[
                ("ccd.pkl", "ccd.pkl"),
                ("boltz1_conf.ckpt", "boltz1_conf.ckpt"),
            ]
        )
    """
    source_dir = os.path.expanduser(source_dir)
    dest_dir = os.path.expanduser(dest_dir)
    os.makedirs(dest_dir, exist_ok=True)

    count = 0
    for dest_name, source_name in file_mappings:
        source_path = os.path.join(source_dir, source_name)
        dest_path = os.path.join(dest_dir, dest_name)

        if link_or_copy(source_path, dest_path, skip_existing=skip_existing):
            count += 1

    return count
