"""Disposable Native Messaging protocol test; no real vault or browser profile."""

import json
import os
from pathlib import Path
import struct
import subprocess
import sys
import tempfile
import time
import unittest


HOST = Path(sys.argv.pop(1)).resolve() if len(sys.argv) > 1 else Path("native-host/build/passage-native-host").resolve()
ORIGIN_A = "chrome-extension://" + "a" * 32 + "/"
ORIGIN_B = "chrome-extension://" + "b" * 32 + "/"


class NativeHostIntegration(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="passage-native-test-", dir=os.environ.get("TMPDIR"))
        self.addCleanup(self.temp.cleanup)
        root = Path(self.temp.name)
        store = root / "store"
        store.mkdir()
        (store / "example.test.gpg").write_bytes(b"encrypted-placeholder")
        bin_dir = root / "bin"
        bin_dir.mkdir()
        self.marker = root / "read-started"
        executable = bin_dir / "pass"
        executable.write_text(
            "#!/bin/sh\n"
            'if [ "$1" = show ]; then\n'
            '  if [ "${PASSAGE_TEST_DELAY:-}" = 1 ]; then touch "$PASSAGE_TEST_MARKER"; sleep 1; fi\n'
            "  printf 'safe\\nlogin: tester\\nurl: https://example.test/login\\n'\n"
            "  exit 0\n"
            "fi\nexit 1\n"
        )
        executable.chmod(0o700)
        self.env = {
            **os.environ,
            "HOME": str(root),
            "XDG_CONFIG_HOME": str(root / "config"),
            "PASSWORD_STORE_DIR": str(store),
            "PASSAGE_TEST_MARKER": str(self.marker),
            "PATH": str(bin_dir) + os.pathsep + os.environ["PATH"],
        }

    def cli(self, *args):
        return subprocess.run([str(HOST), *args], env=self.env, capture_output=True, check=True).stdout

    def frame(self, kind, **fields):
        data = json.dumps({"type": kind, "requestId": "test", **fields}).encode()
        return struct.pack("<I", len(data)) + data

    @staticmethod
    def decode(data):
        size = struct.unpack("<I", data[:4])[0]
        return json.loads(data[4 : 4 + size])

    def request(self, extension_origin, kind, **fields):
        response = subprocess.run(
            [str(HOST), extension_origin], input=self.frame(kind, **fields), env=self.env,
            capture_output=True, check=True,
        )
        return self.decode(response.stdout)

    def pair(self, origin, installation):
        pending = self.request(origin, "pair", installationId=installation)
        self.assertEqual(pending["type"], "pairPending")
        self.cli("approve", pending["code"])
        paired = self.request(origin, "pairStatus", installationId=installation)
        self.assertEqual(paired["type"], "paired")
        return paired["token"]

    def test_two_extension_origins_are_isolated_and_revocable(self):
        token_a = self.pair(ORIGIN_A, "brave-installation")
        fields_a = {"installationId": "brave-installation", "token": token_a, "origin": "https://example.test"}
        self.assertEqual(self.request(ORIGIN_A, "listForOrigin", **fields_a)["entries"][0]["username"], "tester")
        self.assertEqual(self.request(ORIGIN_B, "listForOrigin", **fields_a)["type"], "error")
        self.assertEqual(self.request(ORIGIN_A, "listForOrigin", **{**fields_a, "origin": "https://example.test.evil.invalid"})["entries"], [])

        token_b = self.pair(ORIGIN_B, "chrome-installation")
        fields_b = {"installationId": "chrome-installation", "token": token_b, "origin": "https://example.test"}
        self.cli("revoke", ORIGIN_A, "brave-installation")
        self.assertEqual(self.request(ORIGIN_A, "listForOrigin", **fields_a)["type"], "error")
        self.assertEqual(self.request(ORIGIN_B, "readForOrigin", id="example.test", **fields_b)["password"], "safe")

    def test_lock_persists_and_rejects_read_started_before_lock(self):
        token = self.pair(ORIGIN_A, "installation")
        fields = {"installationId": "installation", "token": token, "origin": "https://example.test", "id": "example.test"}
        self.assertEqual(self.request(ORIGIN_A, "readForOrigin", **fields)["password"], "safe")

        delayed_env = {**self.env, "PASSAGE_TEST_DELAY": "1"}
        process = subprocess.Popen(
            [str(HOST), ORIGIN_A], stdin=subprocess.PIPE, stdout=subprocess.PIPE,
            stderr=subprocess.PIPE, env=delayed_env,
        )
        process.stdin.write(self.frame("readForOrigin", **fields))
        process.stdin.close()
        process.stdin = None
        for _ in range(100):
            if self.marker.exists():
                break
            time.sleep(0.01)
        self.assertTrue(self.marker.exists(), "read did not begin")
        self.cli("lock")
        self.assertEqual(json.loads(self.cli("lock-status")), {"locked": True})
        output, _ = process.communicate(timeout=5)
        self.assertEqual(self.decode(output)["type"], "error")
        self.assertEqual(process.returncode, 0)
        self.assertEqual(self.request(ORIGIN_A, "status", installationId="installation")["locked"], True)
        self.cli("unlock")
        self.assertEqual(self.request(ORIGIN_A, "readForOrigin", **fields)["password"], "safe")


if __name__ == "__main__":
    unittest.main()
