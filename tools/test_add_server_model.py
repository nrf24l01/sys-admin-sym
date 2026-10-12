import importlib.util
import json
from pathlib import Path
import tempfile
import unittest


class AddServerModelTests(unittest.TestCase):
    def setUp(self):
        spec = importlib.util.spec_from_file_location(
            "add_server_model", Path(__file__).with_name("add_server_model.py")
        )
        self.module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(self.module)
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.path = Path(self.tmp.name) / "servers.json"
        self.source = {
            "models": [{"id": "original", "manufacturer": "Vendor",
                        "display_name": {"en": "Old name", "ru": "Old name"},
                        "chassis": {"model": "Old name", "cpu_socket": "LGA4677"},
                        "artwork": {"front": {"path": "old_front.png", "uv": [0, 0, 1, 1]},
                                    "rear": {"path": "old_rear.png", "uv": [0, 0, 1, 1]}}}]
        }
        self.path.write_text(json.dumps(self.source))

    def add(self, template="original", model_id="new_server"):
        return self.module.clone_model(self.path, template, model_id, "Example",
                                       "Example 2U", "equipment/new_front.png",
                                       "equipment/new_rear.png")

    def test_clone_preserves_template_and_changes_identity_and_artwork(self):
        self.add()
        models = json.loads(self.path.read_text())["models"]
        self.assertEqual(models[0], self.source["models"][0])
        new = models[1]
        self.assertEqual(new["id"], "new_server")
        self.assertEqual(new["manufacturer"], "Example")
        self.assertEqual(new["display_name"], {"en": "Example 2U", "ru": "Example 2U"})
        self.assertEqual(new["chassis"]["model"], "Example 2U")
        self.assertEqual(new["chassis"]["cpu_socket"], "LGA4677")
        self.assertEqual(new["artwork"]["front"]["path"], "equipment/new_front.png")

    def test_duplicate_id_and_unknown_template_preserve_file(self):
        before = self.path.read_bytes()
        for template, model_id in [("original", "original"), ("unknown", "new")]:
            with self.assertRaises(ValueError):
                self.add(template, model_id)
            self.assertEqual(self.path.read_bytes(), before)

    def test_invalid_id_and_artwork_path_preserve_file(self):
        before = self.path.read_bytes()
        with self.assertRaises(ValueError):
            self.add(model_id="bad id")
        with self.assertRaises(ValueError):
            self.module.clone_model(self.path, "original", "new", "Example", "Name",
                                    "../outside.png", "rear.png")
        self.assertEqual(self.path.read_bytes(), before)
