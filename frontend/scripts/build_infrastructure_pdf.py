"""Build the System Infrastructure PDF shown in the operator console.

    python frontend/scripts/build_infrastructure_pdf.py [--chrome PATH]

Renders resources/infrastructure/system-infrastructure.html with headless
Chrome or Edge (vector output: every diagram is SVG, so it stays sharp at any
zoom), then:

* sets the document metadata (needs `pypdf`, optional),
* checks the page count and page sizes,
* renders preview images of key pages (needs `pymupdf`, optional),
* writes manifest.json with the version, page count, size and SHA-256, which
  the operator console shows next to the download button.

Run it again whenever the HTML changes, and ship the resulting files with
the frontend.
"""
from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import os
import pathlib
import shutil
import subprocess
import sys
import tempfile

HERE = pathlib.Path(__file__).resolve().parent
DOC_DIR = HERE.parent / "resources" / "infrastructure"
SOURCE = DOC_DIR / "system-infrastructure.html"
OUTPUT = DOC_DIR / "Mailvora-System-Infrastructure.pdf"
MANIFEST = DOC_DIR / "manifest.json"
VERSION = "1.0"
TITLE = "Mailvora System Infrastructure"

CANDIDATES = [
    r"C:\Program Files\Google\Chrome\Application\chrome.exe",
    r"C:\Program Files (x86)\Google\Chrome\Application\chrome.exe",
    r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe",
    r"C:\Program Files\Microsoft\Edge\Application\msedge.exe",
    "/usr/bin/google-chrome", "/usr/bin/chromium", "/usr/bin/chromium-browser", "/usr/bin/microsoft-edge",
    "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
]
# Pages to preview, found by a phrase on the page.
PREVIEWS = {"cover": "Pre-production baseline", "deployment": "Figure 2.", "security": "Figure 7."}


def find_browser(explicit: str | None) -> str:
    for path in [explicit, os.environ.get("CHROME")] + CANDIDATES:
        if path and pathlib.Path(path).is_file():
            return path
    for name in ("google-chrome", "chromium", "chrome", "msedge"):
        if found := shutil.which(name):
            return found
    sys.exit("No Chrome or Edge found. Pass --chrome PATH or set CHROME.")


def render(browser: str, target: pathlib.Path) -> None:
    profile = tempfile.mkdtemp(prefix="mv-pdf-")
    try:
        cmd = [browser, "--headless=new", "--disable-gpu", "--no-first-run", "--no-default-browser-check",
               f"--user-data-dir={profile}", "--no-pdf-header-footer", "--virtual-time-budget=8000",
               "--run-all-compositor-stages-before-draw", f"--print-to-pdf={target}", SOURCE.as_uri()]
        subprocess.run(cmd, check=True, timeout=180, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    finally:
        shutil.rmtree(profile, ignore_errors=True)
    if not target.is_file() or target.stat().st_size < 10_000:
        sys.exit("The browser did not produce a PDF.")


def finish(raw: pathlib.Path) -> dict:
    info = {"pages": None, "landscape_pages": []}
    try:
        from pypdf import PdfReader, PdfWriter
    except ImportError:
        print("pypdf not installed: metadata not set (pip install pypdf)")
        shutil.move(raw, OUTPUT)
        return info
    reader = PdfReader(raw)
    writer = PdfWriter(clone_from=reader)
    writer.add_metadata({
        "/Title": TITLE,
        "/Author": "Mailvora · Powered by TECHCORE SOLUTIONS",
        "/Subject": "Architecture, system flows and security flows of the Mailvora platform",
        "/Keywords": "Mailvora, infrastructure, architecture, security, data flow, email platform",
        "/Creator": "Mailvora documentation build",
    })
    with open(OUTPUT, "wb") as fh:
        writer.write(fh)
    info["pages"] = len(reader.pages)
    info["landscape_pages"] = [i + 1 for i, p in enumerate(reader.pages) if float(p.mediabox.width) > float(p.mediabox.height)]
    raw.unlink(missing_ok=True)
    return info


def previews() -> list[str]:
    try:
        import pymupdf
    except ImportError:
        try:
            import fitz as pymupdf  # older PyMuPDF
        except ImportError:
            print("pymupdf not installed: previews skipped (pip install pymupdf)")
            return []
    made = []
    with pymupdf.open(OUTPUT) as doc:
        for name, phrase in PREVIEWS.items():
            for page in doc:
                if phrase in page.get_text():
                    out = DOC_DIR / f"preview-{name}.png"
                    page.get_pixmap(dpi=110).save(out)
                    made.append(out.name)
                    break
    return made


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--chrome", help="path to Chrome or Edge")
    args = ap.parse_args()
    browser = find_browser(args.chrome)
    raw = DOC_DIR / "build.tmp.pdf"
    print(f"Rendering {SOURCE.name} with {pathlib.Path(browser).name} ...")
    render(browser, raw)
    info = finish(raw)
    images = previews()
    data = OUTPUT.read_bytes()
    manifest = {
        "title": TITLE,
        "version": VERSION,
        "built_at": dt.datetime.now(dt.timezone.utc).replace(microsecond=0).isoformat(),
        "file": OUTPUT.name,
        "bytes": len(data),
        "sha256": hashlib.sha256(data).hexdigest(),
        "pages": info["pages"],
        "landscape_pages": info["landscape_pages"],
        "previews": images,
    }
    MANIFEST.write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
    print(f"Wrote {OUTPUT.name}: {manifest['pages']} pages, {len(data) / 1024:.0f} KB, sha256 {manifest['sha256'][:16]}...")
    if images:
        print("Previews:", ", ".join(images))
    return 0


if __name__ == "__main__":
    sys.exit(main())
