"""Small, reproducible art trials. Credentials are read from the environment only.

Examples:
  python tools/graphics_trial.py estimate-retro
  python tools/graphics_trial.py generate pixellab frog
  python tools/graphics_trial.py generate retrodiffusion frog

Generation commands spend the provider's existing credits or generations.
Each invocation makes one submission and never automatically retries it.
"""

import argparse
import base64
import datetime
from io import BytesIO
import json
import os
from pathlib import Path
import time
import urllib.error
import urllib.request
import uuid
from zipfile import ZipFile


ROOT = Path(__file__).resolve().parents[1]
RECIPE_PATH = ROOT / "art" / "trial-recipe.json"
RECIPE = json.loads(RECIPE_PATH.read_text(encoding="utf-8"))
SAMPLES = RECIPE["samples"]
CARDINALS = ("north", "south", "west", "east")


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        return None


def safe_json(value):
    text = json.dumps(value, indent=2)
    for name in ("PIXELLAB_API_KEY", "RETRODIFFUSION_API_KEY"):
        secret = os.getenv(name)
        if secret:
            text = text.replace(secret, "[REDACTED]")
    return text


def request(provider, path, payload=None, *, binary=False):
    if provider == "pixellab":
        host = "https://api.pixellab.ai/v2"
        headers = {"Authorization": "Bearer " + os.environ["PIXELLAB_API_KEY"]}
    else:
        host = "https://api.retrodiffusion.ai/v2"
        headers = {"X-RD-Token": os.environ["RETRODIFFUSION_API_KEY"]}
    headers["Content-Type"] = "application/json"
    # Persisted before submission; re-use only for the same RetroDiffusion input.
    if provider == "retrodiffusion" and payload is not None and not payload.get("check_cost"):
        headers["Idempotency-Key"] = RUN_ID
    req = urllib.request.Request(host + path, data=json.dumps(payload).encode() if payload is not None else None, headers=headers)
    with urllib.request.build_opener(NoRedirect).open(req, timeout=180) as response:
        return response.status, response.read() if binary else json.load(response)


def build_payload(provider, sample):
    spec = SAMPLES[sample]
    prompt = RECIPE["style"] + " " + spec["prompt"]
    canvas = spec.get("canvas", RECIPE["canvas"])
    is_sprite = spec["kind"] == "sprite"
    if provider == "pixellab":
        if spec["kind"] == "character":
            return {
                "description": prompt,
                "image_size": dict(canvas),
                "view": "low top-down",
                "proportions": {"type": "preset", "name": "chibi"},
                **spec[provider],
            }
        result = {
            "description": prompt,
            "image_size": dict(canvas),
            "detail": "medium detail",
            "outline": "selective outline",
            "no_background": is_sprite,
            "seed": 371,
            "enhance_prompt": False,
        }
        if is_sprite:
            result.update(view="low top-down", direction="south")
        return {**result, **spec.get(provider, {})}
    return {
        "prompt": prompt,
        # Exact style identifier returned by this account's styles/selector.
        "prompt_style": "rd_fast__low_res",
        "width": canvas["width"],
        "height": canvas["height"],
        "num_images": 1,
        "seed": 371,
        "remove_bg": is_sprite,
        **spec.get(provider, {}),
    }


def main():
    global RUN_ID
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=["estimate-retro", "generate"])
    parser.add_argument("provider", nargs="?", choices=["pixellab", "retrodiffusion"])
    parser.add_argument("sample", nargs="?", choices=SAMPLES, default="frog")
    parser.add_argument("--seed", type=int, help="Override just the seed for a new candidate")
    args = parser.parse_args()
    if args.action == "generate" and not args.provider:
        parser.error("generate requires a provider")
    provider = "retrodiffusion" if args.action == "estimate-retro" else args.provider
    character = SAMPLES[args.sample]["kind"] == "character"
    if character and provider != "pixellab":
        parser.error("The directional character recipe uses PixelLab")
    payload = build_payload(provider, args.sample)
    if args.seed is not None:
        payload["seed"] = args.seed
    if args.action == "estimate-retro":
        payload["check_cost"] = True
    RUN_ID = "rainy-day-art-" + uuid.uuid4().hex
    folder = ROOT / "art" / "experiments" / RUN_ID
    folder.mkdir(parents=True, exist_ok=False)
    record = {
        "run_id": RUN_ID,
        "created_at": datetime.datetime.now(datetime.timezone.utc).isoformat(),
        "provider": provider,
        "sample": args.sample,
        "endpoint": (
            "/create-character-with-4-directions" if character else
            "/create-image-pixen" if provider == "pixellab" else "/inferences"
        ),
        "request": payload,
        "status": "not_submitted",
    }
    manifest = folder / "manifest.json"

    def save():
        manifest.write_text(safe_json(record) + "\n")

    save()
    try:
        record["status"] = "submitting"
        save()
        code, data = request(provider, record["endpoint"], payload)
        record["http_status"] = code
        if character:
            record.update(
                background_job_id=data["background_job_id"],
                character_id=data["character_id"],
                response={"usage": data.get("usage")},
                status="pending",
            )
            save()
            for _ in range(90):
                time.sleep(4)
                _, job = request(provider, "/background-jobs/" + record["background_job_id"])
                if job["status"] == "failed":
                    record.update(status="failed", error=job.get("last_response"))
                    break
                if job["status"] == "completed":
                    _, zip_data = request(
                        provider, "/characters/" + record["character_id"] + "/zip", binary=True,
                    )
                    with ZipFile(BytesIO(zip_data)) as exported:
                        outputs = {}
                        for direction in CARDINALS:
                            entry = next(
                                n for n in exported.namelist()
                                if n == f"rotations/{direction}.png"
                                or n.endswith(f"/rotations/{direction}.png")
                            )
                            output = folder / f"{args.sample}-{direction}.png"
                            output.write_bytes(exported.read(entry))
                            outputs[direction] = str(output.relative_to(ROOT))
                    record.update(status="completed", outputs=outputs)
                    if record["response"]["usage"] is None:
                        record["response"]["usage"] = job.get("usage")
                    break
            save()
            print(safe_json({"manifest": str(manifest), **record}), flush=True)
            return int(record["status"] != "completed")
        # Persist the accepted task before polling; never resubmit after a timeout.
        if provider == "retrodiffusion" and data.get("task_id"):
            record.update(task_id=data["task_id"], status="pending")
            save()
            for _ in range(150):
                time.sleep(2)
                _, task = request(provider, "/inferences/tasks/" + data["task_id"])
                if task["status"] == "failed":
                    record.update(status="failed", error=task.get("error"))
                    save()
                    print(safe_json({"manifest": str(manifest), **record}), flush=True)
                    raise SystemExit(1)
                if task["status"] == "succeeded":
                    data = task["result"]
                    break
            else:
                record["status"] = "pending"
                save()
                print(safe_json({"manifest": str(manifest), **record}), flush=True)
                return
        # Save images separately; never print or persist headers or secret values.
        encoded = data.get("image", {}).get("base64")
        if provider == "retrodiffusion" and data.get("base64_images"):
            encoded = data["base64_images"][0]
        raw = None
        if encoded:
            raw = base64.b64decode(encoded.split(",", 1)[-1], validate=True)
        elif provider == "retrodiffusion" and data.get("output_urls"):
            entry = data["output_urls"][0]
            url = entry if isinstance(entry, str) else entry["url"]
            if not url.startswith("https://"):
                raise ValueError("Output download must use HTTPS")
            # Signed output URLs need no API credential. Never forward auth headers.
            with urllib.request.build_opener(NoRedirect).open(url, timeout=60) as response:
                raw = response.read()
        if raw:
            output = folder / (args.sample + ".png")
            output.write_bytes(raw)
            record["output"] = str(output.relative_to(ROOT))
        record["response"] = {k: v for k, v in data.items() if k not in ("image", "base64_images", "output_images", "output_urls")}
        if data.get("model") == "check_cost":
            record["status"] = "estimated"
        else:
            record["status"] = "completed" if record.get("output") else "response_received"
    except urllib.error.HTTPError as error:
        record.update(status="rejected", http_status=error.code)
        try:
            record["error"] = json.loads(error.read(8192))
        except ValueError:
            record["error"] = "Non-JSON error body omitted"
    except Exception as error:
        record.update(status="submission_outcome_uncertain", error_type=type(error).__name__)
        # Do not resubmit automatically: a timed-out generation may still finish.
    save()
    print(safe_json({"manifest": str(manifest), **record}), flush=True)
    if record["status"] in ("rejected", "submission_outcome_uncertain"):
        raise SystemExit(1)


if __name__ == "__main__":
    raise SystemExit(main())
