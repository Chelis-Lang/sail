#!/usr/bin/env python3
"""Exercise the real local wrappers without building or invoking Docker."""
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parent.parent


class LocalTargetTests(unittest.TestCase):
    def test_prepared_checkouts_cannot_share_build_fingerprints(self):
        with tempfile.TemporaryDirectory(prefix="sail-local-target-") as temporary:
            base = Path(temporary)
            tools = base / "bin"
            tools.mkdir()
            docker = tools / "docker"
            docker.write_text("""#!/usr/bin/env python3
import json, os, sys
with open(os.environ['SAIL_LOCAL_TARGET_TEST_LOG'], 'a') as out:
    out.write(json.dumps(sys.argv[1:]) + '\\n')
""")
            docker.chmod(0o755)
            checkouts = [base / name / "source" for name in ("a", "b")]
            for checkout in checkouts:
                (checkout / "ci").mkdir(parents=True)
                for name in ("local.sh", "local-singular.sh", "target-volume.sh"):
                    shutil.copy2(ROOT / "ci" / name, checkout / "ci" / name)
                    os.utime(checkout / "ci" / name, (1_600_000_000, 1_600_000_000))
                tag = checkout / "ci/image-tag.sh"
                tag.write_text("#!/usr/bin/env bash\necho fixture-image\n")
                tag.chmod(0o755)
            alias = base / "alias"
            alias.symlink_to(checkouts[0], target_is_directory=True)
            log = base / "docker.jsonl"
            env = dict(os.environ, PATH=str(tools) + os.pathsep + os.environ["PATH"],
                       SAIL_LOCAL_TARGET_TEST_LOG=str(log))
            env.pop("CI", None)
            selected = []
            for wrapper in ("local.sh", "local-singular.sh"):
                volumes = []
                for checkout in (*checkouts, checkouts[0], alias):
                    log.write_text("")
                    subprocess.run(["bash", str(checkout / "ci" / wrapper), "gate"],
                                   env=env, check=True, capture_output=True)
                    calls = [json.loads(line) for line in log.read_text().splitlines()]
                    runs = [call for call in calls if call[0] == "run"]
                    self.assertEqual(len(runs), 1)
                    mounts = [runs[0][i + 1] for i, arg in enumerate(runs[0][:-1])
                              if arg == "-v"]
                    target = next(mount for mount in mounts if mount.endswith(":/work/target"))
                    volume = target.removesuffix(":/work/target")
                    volumes.append(volume)
                self.assertNotEqual(volumes[0], volumes[1], "prepared snapshots need separate targets")
                self.assertEqual(volumes[0], volumes[2], "same checkout keeps its warm target")
                self.assertEqual(volumes[0], volumes[3], "physical aliases keep their warm target")
                for checkout, volume in zip((*checkouts, checkouts[0], alias), volumes):
                    query = subprocess.run(["bash", str(checkout / "ci" / wrapper),
                                            "target-volume"], env=env, check=True,
                                           capture_output=True, text=True).stdout.strip()
                    self.assertEqual(query, volume)
                selected.append(volumes[0])
            self.assertNotEqual(selected[0], selected[1], "Singular keeps its separate target")


if __name__ == "__main__":
    unittest.main()
