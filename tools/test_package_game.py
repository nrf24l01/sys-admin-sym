import os
import tempfile
import unittest
from pathlib import Path
from zipfile import ZipFile

from package_game import package_game


class PackageGameTest(unittest.TestCase):
    def test_zip_contains_executable_assets_and_readme_without_save_data(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            executable = root / "cloud-provider-sim"
            executable.write_bytes(b"binary")
            executable.chmod(0o755)
            terminal = root / "game-ssh"
            terminal.write_bytes(b"terminal binary")
            terminal.chmod(0o755)
            assets = root / "assets"
            (assets / "equipment").mkdir(parents=True)
            (assets / "equipment" / "server.png").write_bytes(b"image")
            (root / "README.md").write_text("Instructions", encoding="utf-8")
            (root / "cloud-provider-save.db").write_bytes(b"private save")
            (root / "cloud-provider-settings.json").write_text('{"password":"private"}', encoding="utf-8")
            output = root / "dist" / "game.zip"

            package_game(executable, terminal, assets, root / "README.md", output)

            with ZipFile(output) as archive:
                self.assertEqual(
                    set(archive.namelist()),
                    {
                        "cloud-provider-sim/cloud-provider-sim",
                        "cloud-provider-sim/game-ssh",
                        "cloud-provider-sim/README.md",
                        "cloud-provider-sim/assets/equipment/server.png",
                    },
                )
                self.assertEqual(archive.read("cloud-provider-sim/assets/equipment/server.png"), b"image")
                if os.name != "nt":
                    self.assertTrue(archive.getinfo("cloud-provider-sim/cloud-provider-sim").external_attr >> 16 & 0o111)
                    self.assertTrue(archive.getinfo("cloud-provider-sim/game-ssh").external_attr >> 16 & 0o111)


if __name__ == "__main__":
    unittest.main()
