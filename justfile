# Use PowerShell on Windows (bash resolves to WSL on some machines)
set windows-shell := ["powershell.exe", "-NoLogo", "-Command"]

# Run all checks (what CI runs)
check: fmt-check clippy test doc doc-python

# Format check (nightly)
fmt-check:
    cargo +nightly fmt --check

# Format (nightly)
fmt:
    cargo +nightly fmt

# Clippy with all targets
clippy:
    cargo clippy --all-targets --all-features -- -D warnings

# Run tests (default features). Portable: needs no Python.
# NOT --all-features, because `extension-module` makes pyo3 omit the libpython
# link, so a standalone test binary cannot resolve Py* symbols. The Python
# bindings are tested separately by `test-python`.
test:
    cargo test

# Run the Python-binding tests. Requires a shared, embeddable CPython; pin it
# explicitly so pyo3 doesn't auto-grab an inconsistent interpreter (e.g. a
# pixi/uv env) whose libpython isn't on the runtime path. `auto-initialize`
# gives the in-process tests an interpreter to attach to.
test-python python="python3":
    PYO3_PYTHON=`which {{python}}` cargo test --features python,pyo3/auto-initialize

# Build docs and check for warnings
doc $RUSTDOCFLAGS="-D warnings":
    cargo doc --no-deps --document-private-items

# Build docs for the pyo3 surface and check for warnings. The featureless `doc`
# recipe never compiles `src/python.rs`, so intra-doc links in that module go
# unchecked unless we build with the feature enabled.
doc-python $RUSTDOCFLAGS="-D warnings":
    cargo doc --no-deps --document-private-items --features python

# Dependency audit
deny:
    cargo deny check

# Check for unused dependencies
machete:
    cargo machete

# File-length gate (max 800 lines). Shares one implementation with the CI job.
file-lengths:
    python3 scripts/check_file_lengths.py

# Packaging check. Not part of `check-all`: `cargo publish --dry-run` refuses to
# run on a dirty tree, and check-all must stay usable mid-edit. Run before tagging.
# NB: this does NOT detect that the version is already on crates.io -- it packages
# and then aborts at the upload step without ever querying the registry.
publish-check:
    cargo publish --dry-run

# Count clippy errors
errors:
    cargo clippy --all-targets --all-features 2>&1 | rg '^error' | wc -l

# Count clippy warnings
warnings:
    cargo clippy --all-targets --all-features 2>&1 | rg '^warning' | wc -l

# Clippy violations per-rule, per-module (optionally filter by dir)
# Usage: just lint [dir]
# Examples: just lint          (all modules)
#           just lint abi      (only src/abi*)
lint dir="":
    #!/usr/bin/env bash
    cargo clippy --all-targets --all-features --message-format=json 2>/dev/null \
    | python3 -c "
    import sys, json
    filt = '{{dir}}'
    seen = set()
    rows = []
    for line in sys.stdin:
        line = line.strip()
        if not line:
            continue
        try:
            msg = json.loads(line)
        except ValueError:
            continue
        if msg.get('reason') != 'compiler-message':
            continue
        m = msg['message']
        code = m.get('code')
        if not code or not code.get('code'):
            continue
        rule = code['code']
        spans = m.get('spans', [])
        primary = next((s for s in spans if s.get('is_primary')), None)
        if not primary:
            continue
        f = primary['file_name']
        # skip non-source files (Cargo.toml metadata lints, etc.)
        if not f.startswith('src/'):
            continue
        if filt and not f.startswith('src/' + filt):
            continue
        # deduplicate across targets (lib vs test emit the same diagnostic)
        key = (f, primary.get('line_start'), rule)
        if key in seen:
            continue
        seen.add(key)
        # module = first two path components under src/
        parts = f.split('/')
        if len(parts) >= 3:
            mod_name = parts[1] + '/' + parts[2].replace('.rs', '')
        elif len(parts) == 2:
            mod_name = parts[1].replace('.rs', '')
        else:
            mod_name = f
        rows.append((mod_name, rule))
    if not rows:
        print('No violations found.')
        sys.exit(0)
    # Count per (module, rule)
    from collections import Counter
    counts = Counter(rows)
    by_mod = {}
    for (mod_name, rule), n in counts.items():
        by_mod.setdefault(mod_name, []).append((rule, n))
    total = sum(counts.values())
    for mod_name in sorted(by_mod):
        rules = sorted(by_mod[mod_name], key=lambda x: -x[1])
        mod_total = sum(n for _, n in rules)
        print(f'\n  {mod_name} ({mod_total})')
        for rule, n in rules:
            print(f'    {n:3d}  {rule}')
    print(f'\n  total: {total}')
    "

# Regenerate Python protobuf bindings (plugin_pb2.py).
# Rust regens automatically via build.rs (prost_build::compile_protos on cargo build).
# Python has no auto-regen trigger, so this task handles it. Requires `protoc`
# on PATH (e.g. `brew install protobuf` / `apt install protobuf-compiler`).
generate-proto:
    protoc --proto_path=proto \
        --python_out=python/foldit_plugin_sdk/proto \
        proto/plugin.proto

# Fail if the committed plugin_pb2.py differs from a fresh protoc run, i.e.
# someone edited proto/plugin.proto without rerunning `generate-proto`.
# Requires `protoc` on PATH, same as `generate-proto`.
proto-drift: generate-proto
    git diff --exit-code -- python/foldit_plugin_sdk/proto/plugin_pb2.py

# Run everything including optional tools
check-all: check test-python deny machete file-lengths
