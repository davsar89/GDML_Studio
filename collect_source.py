"""Collect all source code files into a single Markdown file for sharing with an AI chatbot."""

import os
from pathlib import Path

# Extensions to include (actual source code + key configs)
SOURCE_EXTENSIONS = {
    ".rs",
    ".ts", ".tsx", ".js", ".jsx",
    ".css", ".html",
    ".toml", ".json",
}

# Directories to skip
SKIP_DIRS = {"node_modules", "target", ".git", "dist", "build", "sample_data"}

# Files to skip (large/noisy files that aren't useful for understanding the code)
SKIP_FILES = {"package-lock.json", "Cargo.lock"}

# Map extensions to markdown language hints
LANG_MAP = {
    ".rs": "rust",
    ".toml": "toml",
    ".ts": "typescript",
    ".tsx": "tsx",
    ".js": "javascript",
    ".jsx": "jsx",
    ".css": "css",
    ".html": "html",
    ".json": "json",
}


def collect_files(root: Path) -> list[Path]:
    files = []
    for dirpath, dirnames, filenames in os.walk(root):
        dirnames[:] = sorted(d for d in dirnames if d not in SKIP_DIRS)
        for f in sorted(filenames):
            p = Path(dirpath) / f
            if p.suffix in SOURCE_EXTENSIONS and f not in SKIP_FILES:
                files.append(p)
    return files


def main():
    root = Path(__file__).resolve().parent
    output = root / "all_source.md"
    files = collect_files(root)

    with open(output, "w", encoding="utf-8") as out:
        out.write("# GDML Studio - Full Source Code\n\n")

        # Table of contents
        out.write("## Table of Contents\n\n")
        for filepath in files:
            rel = filepath.relative_to(root).as_posix()
            anchor = rel.replace("/", "").replace(".", "").replace("_", "").lower()
            out.write(f"- [`{rel}`](#{anchor})\n")
        out.write(f"\n---\n\n")

        # File contents
        for filepath in files:
            rel = filepath.relative_to(root).as_posix()
            lang = LANG_MAP.get(filepath.suffix, "")
            out.write(f"## `{rel}`\n\n")
            try:
                content = filepath.read_text(encoding="utf-8")
            except Exception as e:
                out.write(f"*Error reading file: {e}*\n\n---\n\n")
                continue
            out.write(f"```{lang}\n{content}\n```\n\n---\n\n")

    print(f"Written {len(files)} files to {output}")


if __name__ == "__main__":
    main()
