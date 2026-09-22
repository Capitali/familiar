import json
import http.server
import os
import subprocess
import tempfile
import threading
import unittest
from pathlib import Path


SCRIPT = Path(__file__).with_name("virtual_home.py")
FEED = Path(__file__).with_name("virtual-home-feed.sh")
DECLARATION = Path(__file__).with_name("actuators.json")


class VirtualHomeTest(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()

    def tearDown(self):
        self.temp.cleanup()

    def run_home(self, *args):
        return subprocess.run(
            ["python3", str(SCRIPT), "--state-dir", self.temp.name, *args],
            check=True,
            text=True,
            capture_output=True,
        ).stdout

    def test_each_surface_is_persistent_and_reports_the_actuator_contract(self):
        before = self.run_home("surface", "living-room-lights", "state")
        self.assertIn("light mode : 0x01 Virtual Light", before)
        self.assertIn("(100%)", before)
        changed = self.run_home("surface", "living-room-lights", "dim")
        self.assertIn("(25%)", changed)
        after = self.run_home("surface", "living-room-lights", "state")
        self.assertEqual(changed, after)

    def test_observations_name_both_surfaces_and_their_aggregate_draw(self):
        self.run_home("surface", "living-room-lights", "dim")
        lines = [json.loads(line) for line in self.run_home("observations").splitlines()]
        self.assertEqual(len(lines), 3)
        self.assertEqual(
            {line["actor"] for line in lines},
            {
                "virtual-home:greenhouse",
                "virtual-home:living-room",
                "virtual-home:energy-meter",
            },
        )
        self.assertIn("lighting-draw:3W", {line["object"] for line in lines})
        self.assertTrue(all(line["action"] == "reports" for line in lines))

    def test_corrupt_existing_state_is_never_replaced_with_a_guess(self):
        Path(self.temp.name, "state.json").write_text('{"living-room-lights":"purple"}')
        result = subprocess.run(
            [
                "python3",
                str(SCRIPT),
                "--state-dir",
                self.temp.name,
                "surface",
                "living-room-lights",
                "state",
            ],
            text=True,
            capture_output=True,
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("unknown state", result.stderr)

    def test_declaration_owns_its_reading_contract_and_revert_map(self):
        declared = json.loads(DECLARATION.read_text())["actuators"]
        self.assertEqual(len(declared), 2)
        for surface in declared:
            fields = surface["state"]["fields"]
            self.assertEqual(fields["power"]["kind"], "enum")
            self.assertEqual(fields["level"]["unit"], "percent")
            self.assertEqual(fields["level"]["min"], 0)
            self.assertEqual(fields["level"]["max"], 100)
            buckets = {bucket["name"] for bucket in surface["buckets"]}
            self.assertEqual(buckets, set(surface["actions"]))
            self.assertEqual(buckets, {"off", "dim", "bright"})

    def test_feed_posts_only_when_the_snapshot_changes(self):
        received = []

        class Observe(http.server.BaseHTTPRequestHandler):
            def do_POST(self):  # noqa: N802
                length = int(self.headers["Content-Length"])
                received.append(json.loads(self.rfile.read(length)))
                self.send_response(200)
                self.end_headers()

            def log_message(self, *_args):
                pass

        server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Observe)
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        env = os.environ.copy()
        env.update(
            {
                "VIRTUAL_HOME_STATE_DIR": self.temp.name,
                "VIRTUAL_HOME_PROGRAM": str(SCRIPT),
                "FAMILIAR_LOCAL_PORT": str(server.server_port),
            }
        )
        try:
            subprocess.run(["bash", str(FEED)], check=True, env=env)
            self.assertEqual(len(received), 3)
            subprocess.run(["bash", str(FEED)], check=True, env=env)
            self.assertEqual(len(received), 3, "an unchanged minute is quiet")
            self.run_home("surface", "greenhouse-lights", "bright")
            subprocess.run(["bash", str(FEED)], check=True, env=env)
            self.assertEqual(len(received), 6)
            self.assertTrue(all(item["action"] == "reports" for item in received))
        finally:
            server.shutdown()
            server.server_close()


if __name__ == "__main__":
    unittest.main()
