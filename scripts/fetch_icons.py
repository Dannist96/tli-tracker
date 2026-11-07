"""Downloads item icons once, at build-prep time, into public/icons/.

The app never loads images from the network at runtime; icons ship with it.
Only image bytes with a valid WEBP/PNG signature are kept.

Run with: python -I scripts/fetch_icons.py
"""

import io
import json
import sys
import urllib.request
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SRC = ROOT / "data-src" / "etor_items_cn.json"
ITEMS = ROOT / "src" / "data" / "items.json"
OUT = ROOT / "public" / "icons"
CDN = "https://cdn.tlidb.com/"
MAX_BYTES = 512 * 1024
ICON_SIZE = 64


def is_image(data: bytes) -> bool:
    return (data[:4] == b"RIFF" and data[8:12] == b"WEBP") or data[:8] == b"\x89PNG\r\n\x1a\n"


def shrink(data: bytes) -> bytes:
    """Re-encodes to a small WEBP; falls back to the original bytes without Pillow."""
    try:
        from PIL import Image
    except ImportError:
        return data
    img = Image.open(io.BytesIO(data)).convert("RGBA")
    img.thumbnail((ICON_SIZE, ICON_SIZE), Image.LANCZOS)
    out = io.BytesIO()
    img.save(out, "WEBP", quality=88, method=6)
    return out.getvalue()


def fetch(job):
    url, dest = job
    if dest.exists():
        return "skip"
    try:
        req = urllib.request.Request(url, headers={"User-Agent": "tli-tracker-build"})
        with urllib.request.urlopen(req, timeout=20) as resp:
            data = resp.read(MAX_BYTES + 1)
        if len(data) > MAX_BYTES or not is_image(data):
            return "bad"
        dest.write_bytes(shrink(data))
        return "ok"
    except Exception:
        return "fail"


def main() -> int:
    OUT.mkdir(parents=True, exist_ok=True)
    items = json.loads(ITEMS.read_text(encoding="utf-8"))
    jobs = []
    for rec in json.loads(SRC.read_text(encoding="utf-8")):
        item = items.get(str(rec.get("id")))
        url = rec.get("img", "")
        if item and item["i"] and url.startswith(CDN):
            jobs.append((url, OUT / item["i"]))
    with ThreadPoolExecutor(max_workers=8) as pool:
        results = list(pool.map(fetch, jobs))
    # Shrink anything downloaded by an older version of this script.
    from PIL import Image

    for path in OUT.glob("*.*"):
        with Image.open(path) as img:
            too_big = max(img.size) > ICON_SIZE
        if too_big:
            path.write_bytes(shrink(path.read_bytes()))
    print({k: results.count(k) for k in set(results)})
    return 0


if __name__ == "__main__":
    sys.exit(main())
