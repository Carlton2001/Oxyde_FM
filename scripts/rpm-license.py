#!/usr/bin/env python3
"""Prints the SPDX License tag of the Linux binaries: the licenses of Oxyde, of the
Rust crates they link on x86_64 Linux (build and dev dependencies excluded) and of
the embedded assets."""
import json
import subprocess
import sys

# Embedded Inter and JetBrains Mono, Lucide, Syntect grammars; see licenses/dependencies.
ASSETS = ["OFL-1.1", "ISC", "MIT"]

metadata = json.loads(subprocess.check_output([
    "cargo", "metadata", "--locked", "--format-version", "1",
    "--filter-platform", "x86_64-unknown-linux-gnu",
]))
packages = {package["id"]: package for package in metadata["packages"]}
nodes = {node["id"]: node for node in metadata["resolve"]["nodes"]}
pending = [i for i, package in packages.items() if package["name"] == "oxyde"]
linked = set()
while pending:
    current = pending.pop()
    if current in linked:
        continue
    linked.add(current)
    for dependency in nodes[current]["deps"]:
        if any(kind["kind"] is None for kind in dependency["dep_kinds"]):
            pending.append(dependency["pkg"])

expressions = set(ASSETS)
for current in linked:
    package = packages[current]
    if not package["license"]:
        sys.exit(f"{package['name']} {package['version']} declares no SPDX license.")
    expression = " ".join(package["license"].replace("/", " OR ").split())
    if "(" not in expression and " OR " not in expression:
        expressions.update(expression.split(" AND "))
    elif "(" not in expression and " AND " not in expression:
        # A plain choice: the same alternatives in any order are one expression.
        expressions.add("(" + " OR ".join(sorted(expression.split(" OR "))) + ")")
    else:
        expressions.add(f"({expression})")
# Oxyde's own license first, then a stable order.
expressions.discard("GPL-3.0-only")
print(" AND ".join(["GPL-3.0-only", *sorted(expressions)]))
