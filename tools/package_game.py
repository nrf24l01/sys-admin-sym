"""Build a portable game ZIP with the executable and runtime assets."""

import argparse
from pathlib import Path
from zipfile import ZIP_DEFLATED, ZipFile


def package_game(executable: Path, terminal: Path, assets: Path, readme: Path, output: Path) -> None:
    if not executable.is_file():
        raise FileNotFoundError(executable)
    if not terminal.is_file():
        raise FileNotFoundError(terminal)
    if not assets.is_dir():
        raise FileNotFoundError(assets)
    if not readme.is_file():
        raise FileNotFoundError(readme)

    output.parent.mkdir(parents=True, exist_ok=True)
    with ZipFile(output, "w", compression=ZIP_DEFLATED, compresslevel=6) as archive:
        archive.write(executable, f"cloud-provider-sim/{executable.name}")
        archive.write(terminal, f"cloud-provider-sim/{terminal.name}")
        archive.write(readme, "cloud-provider-sim/README.md")
        for path in sorted(assets.rglob("*")):
            if path.is_file():
                archive.write(path, f"cloud-provider-sim/assets/{path.relative_to(assets).as_posix()}")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--executable", required=True, type=Path)
    parser.add_argument("--terminal", required=True, type=Path)
    parser.add_argument("--assets", required=True, type=Path)
    parser.add_argument("--readme", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    arguments = parser.parse_args()
    package_game(arguments.executable, arguments.terminal, arguments.assets, arguments.readme, arguments.output)
