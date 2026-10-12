#!/usr/bin/env python3
"""Create a server model entry from an existing, verified hardware template."""
import argparse
import copy
import json
import os
from pathlib import Path, PurePosixPath
import re
import tempfile


def clone_model(catalog, template, model_id, manufacturer, name, front, rear):
    catalog = Path(catalog)
    if not re.fullmatch(r"[a-z][a-z0-9_]*", model_id):
        raise ValueError("model ID must use lowercase letters, digits and underscores")
    if not manufacturer.strip() or not name.strip():
        raise ValueError("manufacturer and name must be nonempty")
    for image in (front, rear):
        path = PurePosixPath(image)
        if not image or path.is_absolute() or ".." in path.parts or "\\" in image:
            raise ValueError("artwork paths must be relative to assets without '..'")
    data = json.loads(catalog.read_text(encoding="utf-8"))
    if any(model["id"] == model_id for model in data["models"]):
        raise ValueError(f"model ID already exists: {model_id}")
    source = next((m for m in data["models"] if m["id"] == template), None)
    if source is None:
        raise ValueError(f"unknown template: {template}")
    model = copy.deepcopy(source)
    model.update(id=model_id, manufacturer=manufacturer.strip(),
                 display_name={"en": name.strip(), "ru": name.strip()})
    model["chassis"]["model"] = name.strip()
    model["artwork"]["front"]["path"] = front
    model["artwork"]["rear"]["path"] = rear
    data["models"].append(model)
    temporary = None
    try:
        with tempfile.NamedTemporaryFile(mode="w", encoding="utf-8", dir=catalog.parent,
                                         prefix=catalog.name + ".", delete=False) as output:
            temporary = Path(output.name)
            json.dump(data, output, ensure_ascii=False, indent=2)
            output.write("\n")
        os.chmod(temporary, catalog.stat().st_mode & 0o777)
        os.replace(temporary, catalog)
    finally:
        if temporary is not None:
            temporary.unlink(missing_ok=True)
    return model


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--catalog", type=Path,
                        default=Path(__file__).resolve().parents[1] / "assets/equipment/servers.json")
    for argument in ("template", "id", "manufacturer", "name", "front", "rear"):
        parser.add_argument("--" + argument, required=True)
    args = parser.parse_args()
    try:
        clone_model(args.catalog, args.template, args.id, args.manufacturer,
                    args.name, args.front, args.rear)
    except (ValueError, OSError, KeyError) as error:
        parser.exit(1, f"Cannot add server: {error}\n")
    print(f"Added {args.id} to {args.catalog}")
    print("Update the copied hardware specifications, recipe, descriptions and artwork UVs;")
    print("add image attribution, then run make check.")


if __name__ == "__main__":
    main()
