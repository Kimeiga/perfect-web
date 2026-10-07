#!/usr/bin/env python3
"""The uploads track's image fixtures (track `uploads`, ADR-XXXX).

Real encoders' output, so the header readers in
`spikes/own-renderer/server/src/uploads/sniff.rs` are checked against files
no test of theirs wrote: Pillow's PNG, JPEG (baseline, progressive, and
turned by an Exif `Orientation`), WebP (lossy `VP8 `, lossless `VP8L`, and
extended `VP8X` with alpha) and GIF. The browser suite attaches them too.

Run once, by hand, where Pillow is installed (`python3 -m pip install
pillow`); its output is committed, and `fixtures.json` names each file's
kind and the width and height a browser shows it at. CI never runs this.
"""

import hashlib
import io
import json
import pathlib

from PIL import Image

OUT = pathlib.Path(__file__).resolve().parent.parent / "spikes/own-renderer/e2e/uploads"


def picture(w, h, mode="RGB"):
    img = Image.new(mode, (w, h))
    for x in range(w):
        for y in range(h):
            r, g, b = (x * 255 // max(w - 1, 1), y * 255 // max(h - 1, 1), 128)
            img.putpixel((x, y), (r, g, b, 160) if mode == "RGBA" else (r, g, b))
    return img


def encode(img, fmt, **kw):
    buf = io.BytesIO()
    img.save(buf, fmt, **kw)
    return buf.getvalue()


def main():
    OUT.mkdir(parents=True, exist_ok=True)
    rows = []

    def add(name, kind, shown, data):
        (OUT / name).write_bytes(data)
        row = {"file": name, "kind": kind, "sha256": hashlib.sha256(data).hexdigest()}
        if shown:
            row["width"], row["height"] = shown
        rows.append(row)

    add("six-by-four.png", "png", (6, 4), encode(picture(6, 4), "PNG"))
    add("eight-by-five.jpeg", "jpeg", (8, 5), encode(picture(8, 5), "JPEG", quality=90))
    add(
        "nine-by-seven-progressive.jpeg",
        "jpeg",
        (9, 7),
        encode(picture(9, 7), "JPEG", progressive=True),
    )
    # Stored 8 wide and 5 tall, `Orientation` 6: shown turned, 5 by 8.
    exif = Image.Exif()
    exif[0x0112] = 6
    add(
        "turned-five-by-eight.jpeg",
        "jpeg",
        (5, 8),
        encode(picture(8, 5), "JPEG", exif=exif.tobytes()),
    )
    add("ten-by-six-lossy.webp", "webp", (10, 6), encode(picture(10, 6), "WEBP", quality=80))
    add(
        "seven-by-three-lossless.webp",
        "webp",
        (7, 3),
        encode(picture(7, 3), "WEBP", lossless=True),
    )
    add(
        "twelve-by-nine-alpha.webp",
        "webp",
        (12, 9),
        encode(picture(12, 9, "RGBA"), "WEBP", quality=80),
    )
    add("five-by-nine.gif", "gif", (5, 9), encode(picture(5, 9).convert("P"), "GIF"))
    # Too wide for the feed's declaration (at most 4096 by 4096).
    add("too-wide.png", "png", (4097, 1), encode(Image.new("RGB", (4097, 1)), "PNG"))
    # Not an image at all, named as one.
    add(
        "not-an-image.png",
        None,
        None,
        b"<!doctype html><title>x</title><script>document.title='ran'</script>",
    )
    (OUT / "fixtures.json").write_text(json.dumps(rows, indent=2) + "\n")
    for r in rows:
        print(r["file"], r["kind"], r.get("width"), r.get("height"))


if __name__ == "__main__":
    main()
