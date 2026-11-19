"""Check saved trial PNGs against the art recipe without calling the APIs.

From the project folder, run: python -B tools/review_art.py
Pass manifest paths to review only those candidates. Requires Pillow.
"""

import argparse
import json
from pathlib import Path

from PIL import Image

from graphics_trial import RECIPE, ROOT, SAMPLES


def review_image(path, spec):
    canvas = spec.get("export_canvas", spec.get("canvas", RECIPE["canvas"]))
    expected_size = (canvas["width"], canvas["height"])
    with Image.open(path) as image:
        if image.format != "PNG":
            return f"Expected PNG, got {image.format}"
        if image.size != expected_size:
            return f"Expected {expected_size}, got {image.size}"
        alpha = image.convert("RGBA").getchannel("A")
    bounds = alpha.getbbox()
    if bounds is None:
        return "Image is fully transparent"
    if spec["kind"] in ("sprite", "character"):
        if alpha.getextrema()[0] == 255:
            return "Sprite has no transparency"
        left, top, right, bottom = bounds
        width, height = expected_size
        if left == 0 or top == 0 or right == width or bottom == height:
            return f"Visible pixels touch the canvas edge (bounds {bounds})"
    elif alpha.getextrema()[0] < 255:
        return "Texture has transparent pixels where the canvas should be filled"
    return None


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("manifests", type=Path, nargs="*")
    args = parser.parse_args()
    paths = args.manifests or sorted((ROOT / "art").glob("*/*/manifest.json"))
    checked = blocked = 0
    for path in paths:
        label = str(path)
        try:
            record = json.loads(path.read_text(encoding="utf-8"))
            if record["status"] != "completed":
                if args.manifests:
                    raise ValueError(f"Run status is {record['status']}")
                continue
            spec = SAMPLES[record["sample"].removesuffix("-walk")]
            label = f"{record['provider']} / {record['sample']} / {path.parent.name}"
            outputs = record.get("outputs")
            files = (
                [image for group in outputs.values() for image in ([group] if isinstance(group, str) else group)]
                if outputs else [record["output"]]
            )
            reason = None
            for name in files:
                image_path = (ROOT / name).resolve()
                if not image_path.is_relative_to(ROOT):
                    raise ValueError("Image path is outside the project")
                issue = review_image(image_path, spec)
                if issue:
                    reason = f"{image_path.name}: {issue}" if outputs else issue
                    break
        except (KeyError, OSError, ValueError) as error:
            reason = str(error)
        checked += 1
        if reason:
            blocked += 1
            print(f"Needs fixing | {label} | {reason}")
        elif spec["kind"] == "study":
            print(f"Study only   | {label} | {spec['review']}")
        else:
            print(f"Visual review | {label} | {spec['review']}")
    print(f"Checked {checked} candidates. {blocked} need fixes. Review the others before shipping.")
    return int(blocked > 0 or checked == 0)


if __name__ == "__main__":
    raise SystemExit(main())
