# Use PowerShell on Windows (bash resolves to WSL on some machines)
set windows-shell := ["powershell.exe", "-NoLogo", "-Command"]

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
