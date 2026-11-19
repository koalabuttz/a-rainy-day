"""Pack the reviewed traveler frames for the shared room renderer."""

import argparse
import json
from pathlib import Path

from PIL import Image

from graphics_trial import CARDINALS, ROOT


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("rotations", type=Path, help="Completed four-direction manifest")
    parser.add_argument("walk", type=Path, help="Completed walk animation manifest")
    args = parser.parse_args()
    stills = json.loads(args.rotations.read_text(encoding="utf-8"))
    walk = json.loads(args.walk.read_text(encoding="utf-8"))
    if (ROOT / walk["source_manifest"]).resolve() != args.rotations.resolve():
        parser.error("The walk and rotation manifests must refer to the same character")

    cell = 36
    columns = len(walk["outputs"][CARDINALS[0]]) + 1
    if columns < 2:
        parser.error("The traveler needs at least one walk frame in each direction")
    atlas = Image.new("RGBA", (columns * cell, len(CARDINALS) * cell))
    for row, direction in enumerate(CARDINALS):
        frames = [stills["outputs"][direction], *walk["outputs"][direction]]
        if len(frames) != columns:
            parser.error(f"{direction} has a different number of frames")
        for column, path in enumerate(frames):
            with Image.open(ROOT / path) as image:
                frame = image.convert("RGBA")
            bounds = frame.getbbox()
            if frame.size != (cell, cell) or not bounds or frame.getchannel("A").getextrema()[0] == 255:
                parser.error(f"Expected a nonempty transparent {cell}×{cell} frame at {path}")
            atlas.paste(frame, (column * cell, row * cell))
    output = ROOT / "assets" / "traveler.png"
    atlas.save(output)
    print(f"Packed {atlas.width}×{atlas.height} traveler atlas at {output}")


if __name__ == "__main__":
    main()
