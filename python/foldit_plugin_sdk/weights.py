"""In-process weight downloading for foldit-runner plugins.

ML plugins ship without their multi-GB weights (``<plugin_dir>/assets/
weights/`` is git-ignored). When the weights are absent a plugin fetches
them itself, at runtime, into its own ``cache_dir`` rather than erroring
with "go run a CLI command".

This module is the shared fetch machinery for plugins whose weights are a
table of source URLs (simplefold). Each plugin owns its own URL
table (a list of :class:`WeightSpec`) and drives the download as a STREAM
op so the host renders progress via ``StreamPending``; the query side
reports presence via :func:`status_json`. foundry is the exception: its
weights come from ``foundry_cli.install_model``, not a URL table, so it
calls that directly and only reuses :func:`status_payload` for the query
encoding.

Pure stdlib (``urllib``) so it works in every plugin env without adding a
dependency. Robustness is deliberately minimal:

- **atomic placement**: each file streams to ``<dest>.partial`` and is
  ``os.replace``-d into place only once complete, so an interrupted
  download never leaves a truncated file that looks present.
- **skip-existing**: a file already at its destination is left alone.
- **cancellable**: the caller passes ``should_cancel``; it is checked
  between chunks, so a multi-GB fetch aborts promptly. A partial file is
  removed on cancel.

No checksums (the sources don't advertise them uniformly) and no HTTP
range/resume (a failed file restarts from scratch).
"""

from __future__ import annotations

import json
import os
from dataclasses import dataclass
from pathlib import Path
from typing import Callable, List, Optional
from urllib.request import Request, urlopen

from .logging_config import get_logger
from .proto import plugin_pb2

logger = get_logger(__name__)

# Shared op / query ids. Every ML plugin exposes the same pair so the host
# treats weight provisioning uniformly: a query to ask whether weights are
# present, a stream op to fetch them with progress.
DOWNLOAD_WEIGHTS_OP = "download_weights"
WEIGHTS_STATUS_QUERY = "weights_status"

_USER_AGENT = "foldit-runner/0.1.0"
_CHUNK = 1 << 16  # 64 KiB
# Push a progress update at most this often (bytes), so a slow link still
# moves the bar without flooding the stream slot on a fast one.
_PROGRESS_STRIDE = 4 << 20  # 4 MiB


class WeightDownloadCancelled(Exception):
    """Raised by :func:`download_specs` when ``should_cancel`` fires."""


@dataclass(frozen=True)
class WeightSpec:
    """One downloadable weight file.

    ``subdir`` is relative to the plugin's ``cache_dir`` (=
    ``<plugin_dir>/assets/weights``); the file lands at
    ``<cache_dir>/<subdir>/<name>``. ``label`` is the human-readable id
    used in progress + status (defaults to ``<subdir>/<name>``).
    """

    url: str
    subdir: str
    name: str

    @property
    def label(self) -> str:
        return f"{self.subdir}/{self.name}" if self.subdir else self.name


def dest_path(cache_dir: str, spec: WeightSpec) -> str:
    """Absolute destination path for ``spec`` under ``cache_dir``."""
    return str(Path(cache_dir) / spec.subdir / spec.name)


# Shared registration specs


def download_weights_op_spec() -> "plugin_pb2.PluginOp":
    """The ``download_weights`` STREAM op every ML plugin declares.

    No params (fetches whatever is missing). Modeled as a stream so the
    host renders progress via ``StreamPending``; ``creates_entities`` is
    false: it provisions assets, it does not touch the assembly. On
    completion the plugin returns its unchanged working assembly (the
    protocol's stream terminal requires assembly bytes; the download
    changes no entities).
    """
    return plugin_pb2.PluginOp(
        id=DOWNLOAD_WEIGHTS_OP,
        display_name="Download weights",
        description=(
            "Fetch this plugin's model weights into its local assets "
            "directory. Multi-GB; progress is reported while it runs."
        ),
        kind=plugin_pb2.OP_KIND_STREAM,
        creates_entities=False,
    )


def weights_status_query_spec() -> "plugin_pb2.PluginQuery":
    """The ``weights_status`` query every ML plugin declares.

    No params. Returns the JSON payload produced by :func:`status_json` /
    :func:`status_payload` so the GUI can show readiness and offer the
    download op when weights are absent.
    """
    return plugin_pb2.PluginQuery(
        id=WEIGHTS_STATUS_QUERY,
        display_name="Weights status",
        description="Report whether this plugin's model weights are present.",
    )


def missing_specs(cache_dir: str, specs: List[WeightSpec]) -> List[WeightSpec]:
    """Return the subset of ``specs`` not yet present on disk."""
    return [s for s in specs if not os.path.exists(dest_path(cache_dir, s))]


# Status (the weights_status query)


def status_payload(present: List[str], missing: List[str]) -> bytes:
    """Encode a weights-status reply as UTF-8 JSON.

    Schema (the consuming GUI panel parses this):

    ``{"ready": bool, "present": [label, ...], "missing": [label, ...]}``

    ``ready`` is true iff nothing is missing. Used directly by foundry,
    whose presence check is glob-based rather than URL-table based.
    """
    return json.dumps(
        {"ready": not missing, "present": present, "missing": missing}
    ).encode("utf-8")


def status_json(cache_dir: str, specs: List[WeightSpec]) -> bytes:
    """Encode the presence of every ``spec`` under ``cache_dir``.

    Convenience wrapper over :func:`status_payload` for URL-table plugins
    (simplefold): partitions ``specs`` by on-disk presence.
    """
    present, missing = [], []
    for spec in specs:
        bucket = present if os.path.exists(dest_path(cache_dir, spec)) else missing
        bucket.append(spec.label)
    return status_payload(present, missing)


# Download (the download_weights op)


def download_specs(
    cache_dir: str,
    specs: List[WeightSpec],
    on_progress: Optional[Callable[[float, str], None]] = None,
    should_cancel: Optional[Callable[[], bool]] = None,
) -> None:
    """Fetch every missing file in ``specs`` into ``cache_dir``.

    ``on_progress(fraction, stage)`` is called as the download advances:
    ``fraction`` is overall progress in ``[0.0, 1.0]`` (per-file fraction
    blended across the file count), ``stage`` a human-readable label.
    ``should_cancel()`` is polled between chunks; when it returns true the
    in-flight ``.partial`` is removed and :class:`WeightDownloadCancelled`
    is raised.

    Already-present files are skipped. Raises on any HTTP / IO failure
    (after cleaning up the partial), so the caller can surface a stream
    error.
    """
    todo = missing_specs(cache_dir, specs)
    if not todo:
        if on_progress is not None:
            on_progress(1.0, "weights already present")
        return

    total = len(todo)
    for index, spec in enumerate(todo):
        if should_cancel is not None and should_cancel():
            raise WeightDownloadCancelled()

        def file_progress(file_frac: float, done_mb: float, total_mb: float) -> None:
            if on_progress is None:
                return
            overall = (index + file_frac) / total
            size = f"{done_mb:.0f}/{total_mb:.0f} MB" if total_mb else f"{done_mb:.0f} MB"
            on_progress(overall, f"{spec.label} ({index + 1}/{total}) {size}")

        logger.info("Downloading %s -> %s", spec.url, spec.label)
        _download_one(
            spec.url,
            dest_path(cache_dir, spec),
            file_progress,
            should_cancel,
        )

    if on_progress is not None:
        on_progress(1.0, "download complete")


def _download_one(
    url: str,
    dest: str,
    on_file_progress: Callable[[float, float, float], None],
    should_cancel: Optional[Callable[[], bool]],
) -> None:
    """Stream ``url`` to ``dest`` atomically via a ``.partial`` sibling."""
    dest_dir = os.path.dirname(dest)
    if dest_dir:
        os.makedirs(dest_dir, exist_ok=True)
    partial = dest + ".partial"

    req = Request(url, headers={"User-Agent": _USER_AGENT})
    try:
        with urlopen(req, timeout=60) as response, open(partial, "wb") as out:
            total_bytes = int(response.headers.get("Content-Length", 0))
            total_mb = total_bytes / (1024 * 1024)
            downloaded = 0
            since_report = 0
            while True:
                if should_cancel is not None and should_cancel():
                    raise WeightDownloadCancelled()
                chunk = response.read(_CHUNK)
                if not chunk:
                    break
                out.write(chunk)
                downloaded += len(chunk)
                since_report += len(chunk)
                if since_report >= _PROGRESS_STRIDE:
                    since_report = 0
                    frac = downloaded / total_bytes if total_bytes else 0.0
                    on_file_progress(frac, downloaded / (1024 * 1024), total_mb)
            on_file_progress(1.0, downloaded / (1024 * 1024), total_mb)
        os.replace(partial, dest)
    except BaseException:
        # Cancel, HTTP error, or IO failure: never leave a partial behind.
        try:
            os.remove(partial)
        except OSError:
            pass
        raise
