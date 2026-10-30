#!/usr/bin/env python3
"""Write the third-party notices of the Linux packages, as update-third-party-licenses.ps1
does for Windows: cargo-about's report, then each crate's NOTICE, COPYRIGHT and AUTHORS."""
import json
import re
import subprocess
import sys
from pathlib import Path

NOTICE = re.compile(r"^(NOTICE|NOTICES|COPYRIGHT|AUTHORS)([-_.].*)?$")


def main(output: Path) -> None:
    temporary = output.with_name(output.name + ".tmp")
    subprocess.run(["cargo", "about", "generate", "--locked", "--fail", "--output-file", str(temporary),
                    "scripts/third-party-notices.hbs"], check=True)
    if temporary.stat().st_size == 0:
        sys.exit("Empty third-party notices.")
    metadata = json.loads(subprocess.run(["cargo", "metadata", "--locked", "--format-version", "1"],
                                         check=True, capture_output=True).stdout)
    supplement = ["\n# Additional upstream notices\n\nOriginal NOTICE, COPYRIGHT and AUTHORS files, "
                  "identified by package and source path.\n"]
    for package in sorted(metadata["packages"], key=lambda package: (package["name"], package["version"], package["source"] or "")):
        if not package["source"]:
            continue
        root = Path(package["manifest_path"]).parent
        for file in sorted(path for path in root.rglob("*") if path.is_file() and NOTICE.match(path.name)):
            contents = file.read_text(encoding="utf-8", errors="replace")
            # A fence longer than any run in the upstream text preserves it verbatim.
            fence = "~" * max(3, 1 + max((len(run) for run in re.findall("~+", contents)), default=0))
            supplement.append(f"\n## {package['name']} {package['version']} — {file.relative_to(root)}\n\n"
                              f"Source: {package['source']}\n\n{fence}text\n{contents}\n{fence}\n")
    with temporary.open("a", encoding="utf-8") as notices:
        notices.writelines(supplement)
    temporary.replace(output)


if __name__ == "__main__":
    if len(sys.argv) != 2:
        sys.exit("Usage: linux-third-party-notices.py OUTPUT")
    main(Path(sys.argv[1]))
