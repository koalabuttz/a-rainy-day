"""Build a self-contained HTML version of the room in target/web."""

import base64
import os
from pathlib import Path
import subprocess


def main():
    root = Path(__file__).resolve().parents[1]
    target = root / "target"
    subprocess.run(
        [
            "cargo", "+stable", "build", "--locked", "--release", "-p", "rainy-day-web",
            "--target", "wasm32-unknown-unknown", "--target-dir", str(target),
        ],
        cwd=root,
        check=True,
    )

    web = root / "web"
    wasm = target / "wasm32-unknown-unknown" / "release" / "rainy_day_web.wasm"
    substitutions = {
        "/* ROOM_STYLE */": (web / "room.css").read_text(encoding="utf-8"),
        "/* ROOM_SCRIPT */": (web / "room.js").read_text(encoding="utf-8"),
        "ROOM_WASM": base64.b64encode(wasm.read_bytes()).decode("ascii"),
    }
    html = (web / "index.html").read_text(encoding="utf-8")
    for marker, value in substitutions.items():
        html = html.replace(marker, value)
    output = target / "web" / "index.html"
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(html, encoding="utf-8")
    print(os.fspath(output))


if __name__ == "__main__":
    main()
