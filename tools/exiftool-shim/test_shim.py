#!/usr/bin/env python3
"""Smoke tests for the exiftool shim (no exiftool/etng required)."""

import json
import os
import stat
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
SHIM = HERE / "exiftool"


class ShimTest(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)
        self.log = self.root / "calls.log"
        self.etng = self.root / "etng"
        self.etng.write_text(
            "#!/usr/bin/env python3\n"
            "import json, sys\n"
            "open(" + repr(str(self.log)) + ", 'a').write(json.dumps(sys.argv[1:]) + '\\n')\n"
            "if '-j' in sys.argv:\n"
            "    print(json.dumps([{'ExifIFD:DateTimeOriginal': '2024:05:06 07:08:09'}]))\n",
            encoding="utf-8",
        )
        self.etng.chmod(self.etng.stat().st_mode | stat.S_IXUSR)
        self.env = dict(os.environ)
        self.env["ETNG_BIN"] = str(self.etng)
        self.env["ETNG_SHIM_LOG"] = str(self.root / "shim.log")

    def tearDown(self):
        self.temp.cleanup()

    def test_translates_phtools_script(self):
        script = self.root / "script.txt"
        script.write_text(
            "-MWG:DateTimeOriginal=2024:05:06 07:08:09\n"
            "-MWG:City=Moscow\n"
            "-MWG:Keywords-=alpha\n"
            "-MWG:Keywords+=alpha\n"
            "-MWG:Keywords+=beta\n"
            "-XMP-mwg-coll:Collections+={CollectionName=Event, CollectionURI=uri}\n"
            "photo.jpg\n"
            "-overwrite_original\n"
            "-execute\n",
            encoding="utf-8",
        )
        result = subprocess.run(
            [sys.executable, str(SHIM), "-@", str(script)],
            env=self.env,
            capture_output=True,
            text=True,
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        calls = [json.loads(line) for line in self.log.read_text().splitlines()]
        self.assertEqual(len(calls), 1)
        args = calls[0]
        self.assertIn("-DateTimeOriginal=2024:05:06 07:08:09", args)
        self.assertIn("-City=Moscow", args)
        self.assertIn("-Keywords=alpha", args)
        self.assertIn("-Keywords=beta", args)
        self.assertIn("-Collections=Event|uri", args)
        self.assertEqual(args[-1], "photo.jpg")

    def test_copy_file_modify_date(self):
        script = self.root / "script.txt"
        script.write_text(
            "-FileModifyDate<DateTimeOriginal\nphoto.jpg\n-execute\n",
            encoding="utf-8",
        )
        result = subprocess.run(
            [sys.executable, str(SHIM), "-@", str(script)],
            env=self.env,
            capture_output=True,
            text=True,
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        calls = [json.loads(line) for line in self.log.read_text().splitlines()]
        self.assertTrue(any(call[:1] == ["-j"] for call in calls))
        self.assertTrue(
            any("-FileModifyDate=2024:05:06 07:08:09" in call for call in calls)
        )


if __name__ == "__main__":
    unittest.main()
