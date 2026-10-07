"""Package the standalone game-ssh client and its SHA-256 checksum."""

import argparse
import hashlib
from pathlib import Path
from zipfile import ZIP_DEFLATED, ZipFile


def package_game_ssh(executable: Path, readme: Path, output: Path) -> None:
    for source in (executable, readme):
        if not source.is_file():
            raise FileNotFoundError(source)

    output.parent.mkdir(parents=True, exist_ok=True)
    with ZipFile(output, "w", compression=ZIP_DEFLATED, compresslevel=6) as archive:
        archive.write(executable, f"game-ssh/{executable.name}")
        archive.write(readme, "game-ssh/README.md")
    checksum = hashlib.sha256(output.read_bytes()).hexdigest()
    output.with_suffix(output.suffix + ".sha256").write_text(
        f"{checksum}  {output.name}\n", encoding="utf-8"
    )


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--executable", required=True, type=Path)
    parser.add_argument("--readme", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    arguments = parser.parse_args()
    package_game_ssh(arguments.executable, arguments.readme, arguments.output)
