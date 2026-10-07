import hashlib
from pathlib import Path
import stat
import tempfile
import unittest
from zipfile import ZipFile

from package_game_ssh import package_game_ssh


class StandaloneClientPackageTests(unittest.TestCase):
    def test_packages_only_client_and_instructions_with_valid_checksum(self):
        for binary_name in ("game-ssh", "game-ssh.exe"):
            with self.subTest(binary=binary_name), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                executable = root / binary_name
                executable.write_bytes(b"test executable")
                executable.chmod(0o755)
                readme = root / "instructions.md"
                readme.write_text("Connect to a running game.", encoding="utf-8")
                (root / "cloud-provider-settings.json").write_text('"private"')
                (root / "cloud-provider-save.db").write_bytes(b"private save")
                output = root / "dist" / "game-ssh-test.zip"

                package_game_ssh(executable, readme, output)

                with ZipFile(output) as archive:
                    self.assertEqual(
                        sorted(archive.namelist()),
                        ["game-ssh/README.md", f"game-ssh/{binary_name}"],
                    )
                    self.assertEqual(archive.read(f"game-ssh/{binary_name}"), executable.read_bytes())
                    self.assertEqual(archive.read("game-ssh/README.md"), readme.read_bytes())
                    mode = archive.getinfo(f"game-ssh/{binary_name}").external_attr >> 16
                    self.assertTrue(mode & stat.S_IXUSR)
                checksum = output.with_suffix(".zip.sha256").read_text(encoding="utf-8")
                self.assertEqual(
                    checksum, f"{hashlib.sha256(output.read_bytes()).hexdigest()}  {output.name}\n"
                )

    def test_missing_input_does_not_create_package(self):
        for missing in ("executable", "readme"):
            with self.subTest(missing=missing), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                executable = root / "game-ssh"
                readme = root / "README.md"
                executable.write_bytes(b"executable")
                readme.write_text("Instructions", encoding="utf-8")
                {"executable": executable, "readme": readme}[missing].unlink()
                output = root / "dist" / "client.zip"
                with self.assertRaises(FileNotFoundError):
                    package_game_ssh(executable, readme, output)
                self.assertFalse(output.exists())
                self.assertFalse(output.with_suffix(".zip.sha256").exists())


if __name__ == "__main__":
    unittest.main()
