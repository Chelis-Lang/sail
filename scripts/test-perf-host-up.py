#!/usr/bin/env python3
"""Exercise Linux host launch, regions and failed-launch cleanup with fake tools."""
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

SCRIPT = Path(__file__).with_name("perf-host.sh")

# Every call is logged with the lane state file as it stood at that moment, so
# a test can tell what was recorded before a given AWS operation ran.
FAKE_AWS = '''#!/usr/bin/env python3
import json,os,pathlib,sys
args=sys.argv[1:]
operation=args[3] if len(args)>3 and args[:2]==['ec2','--region'] else 'unexpected'
if operation=='wait':operation+=' '+args[4]
env=pathlib.Path(os.environ['SAIL_PERF_STATE_DIR'])/'fixture.env'
with open(os.environ['SAIL_FAKE_AWS_CALLS'],'a') as f:
 f.write(json.dumps({'args':args,'state':env.read_text() if env.exists() else None})+'\\n')
p=pathlib.Path(os.environ['SAIL_FAKE_AWS_RESPONSES']);responses=json.loads(p.read_text())
rows=responses.get(operation,[])
if not rows:
 print('Unexpected fake AWS command: '+operation,file=sys.stderr);sys.exit(99)
row=rows.pop(0);p.write_text(json.dumps(responses))
print(row.get('out',''),end='')
print(row.get('err',''),end='',file=sys.stderr)
sys.exit(row.get('status',0))
'''

FILES = ["fixture.env", "fixture.pem", "fixture.known_hosts"]


def row(out="", err="", status=0):
    return {"out": out, "err": err, "status": status}


def failure(code, operation):
    return row(err=f"\nAn error occurred ({code}) when calling the {operation} operation: refused\n", status=254)


def launch(architectures="x86_64", **overrides):
    """Responses for one complete launch; overrides replace single operations."""
    responses = {
        "describe-instance-types": [row(architectures + "\n")],
        "describe-images": [row("ami-fixture\n")],
        "describe-vpcs": [row("vpc-fixture\n")],
        "create-key-pair": [row("PRIVATE KEY\n")],
        "create-security-group": [row("sg-new\n")],
        "authorize-security-group-ingress": [row("{}\n")],
        "run-instances": [row("i-new\n")],
        "wait instance-running": [row()],
        "describe-instances": [row("203.0.113.7\n")],
    }
    responses.update(overrides)
    return responses


class Host:
    """One temporary state directory with fake aws, curl, ssh and sleep."""

    def __init__(self, root):
        self.root = Path(root)
        self.state = self.root / "state"
        self.calls = self.root / "calls.jsonl"
        self.responses = self.root / "responses.json"
        self.bin = self.root / "bin"
        self.bin.mkdir()
        self.tool("aws", FAKE_AWS)
        self.tool("curl", "#!/bin/sh\necho 198.51.100.9\n")
        self.tool("sleep", "#!/bin/sh\nexit 0\n")
        self.ssh_status(0)

    def tool(self, name, text):
        path = self.bin / name
        path.write_text(text)
        path.chmod(0o755)

    def ssh_status(self, status):
        self.tool("ssh", f"#!/bin/sh\nexit {status}\n")

    def record(self, text, lane="fixture"):
        self.state.mkdir(exist_ok=True)
        (self.state / f"{lane}.env").write_text(text)

    def run(self, responses, *command, region=None):
        self.responses.write_text(json.dumps(responses))
        if self.calls.exists():
            self.calls.unlink()
        env = dict(os.environ)
        env.pop("AWS_REGION", None)
        if region:
            env["AWS_REGION"] = region
        env.update(PATH=f"{self.bin}:{env['PATH']}", SAIL_PERF_STATE_DIR=str(self.state),
                   SAIL_FAKE_AWS_CALLS=str(self.calls), SAIL_FAKE_AWS_RESPONSES=str(self.responses))
        result = subprocess.run(["bash", str(SCRIPT), *command], env=env,
                                text=True, capture_output=True, timeout=120)
        calls = [json.loads(line) for line in self.calls.read_text().splitlines()] if self.calls.exists() else []
        return result, calls

    def files(self):
        return {name: (self.state / name).exists() for name in FILES}

    def env(self):
        return dict(line.split("=", 1) for line in (self.state / "fixture.env").read_text().splitlines())


def operations(calls):
    return [call["args"][3] for call in calls]


def call_for(calls, operation):
    found = [call for call in calls if call["args"][3] == operation]
    assert len(found) == 1, (operation, found)
    return found[0]


class Launch(unittest.TestCase):
    def setUp(self):
        tmp = tempfile.TemporaryDirectory(prefix="sail-perf-up-")
        self.addCleanup(tmp.cleanup)
        self.host = Host(tmp.name)

    def assert_nothing_left(self, result):
        self.assertNotEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertFalse(any(self.host.files().values()), self.host.files())

    def test_x86_type_launches_amd64_image_and_records_state(self):
        result, calls = self.host.run(launch(), "up", "fixture", "t3.small", region="us-west-2")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("fixture ready: i-new 203.0.113.7", result.stdout)
        self.assertTrue(all(call["args"][2] == "us-west-2" for call in calls), calls)
        images = call_for(calls, "describe-images")["args"]
        self.assertIn("Name=name,Values=ubuntu/images/hvm-ssd-gp3/ubuntu-noble-24.04-amd64-server-*", images)
        self.assertEqual(self.host.env(), {
            "region": "us-west-2", "type": "t3.small", "arch": "amd64", "key": "sail-fixture",
            "sg": "sg-new", "id": "i-new", "addr": "203.0.113.7"})
        self.assertEqual((self.host.state / "fixture.pem").read_text(), "PRIVATE KEY\n")
        self.assertEqual((self.host.state / "fixture.pem").stat().st_mode & 0o777, 0o600)

    def test_graviton_type_launches_arm64_image(self):
        result, calls = self.host.run(launch("arm64"), "up", "fixture", "t4g.small")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("t4g.small", call_for(calls, "describe-instance-types")["args"])
        images = call_for(calls, "describe-images")["args"]
        self.assertIn("Name=name,Values=ubuntu/images/hvm-ssd-gp3/ubuntu-noble-24.04-arm64-server-*", images)
        self.assertEqual(self.host.env()["arch"], "arm64")
        self.assertTrue(all(call["args"][2] == "us-east-1" for call in calls), calls)

    def test_other_architectures_and_unknown_types_create_nothing(self):
        cases = (launch("arm64_mac"), launch("i386"), launch("None"),
                 launch(**{"describe-instance-types": [failure("InvalidInstanceType", "DescribeInstanceTypes")]}),
                 launch(**{"describe-images": [row("None\n")]}))
        for responses in cases:
            with self.subTest(responses=responses):
                result, calls = self.host.run(responses, "up", "fixture", "odd.type")
                self.assert_nothing_left(result)
                self.assertFalse(any(op.startswith(("create-", "run-", "delete-")) for op in operations(calls)), calls)

    def test_region_is_recorded_before_anything_is_created(self):
        _, calls = self.host.run(launch(), "up", "fixture", "t3.small", region="us-west-2")
        self.assertIn("region=us-west-2\n", call_for(calls, "create-key-pair")["state"])
        before_launch = call_for(calls, "run-instances")["state"]
        for line in ("region=us-west-2", "key=sail-fixture", "sg=sg-new"):
            self.assertIn(line, before_launch.splitlines())
        self.assertNotIn("id=", before_launch)

    def test_user_data_installs_unzip(self):
        _, calls = self.host.run(launch(), "up", "fixture", "t3.small")
        arguments = call_for(calls, "run-instances")["args"]
        user_data = arguments[arguments.index("--user-data") + 1]
        install = [line for line in user_data.splitlines() if line.startswith("apt-get install")]
        self.assertEqual(len(install), 1, user_data)
        self.assertIn("unzip", install[0].split())

    def test_refused_launch_deletes_group_key_and_local_files(self):
        responses = launch(**{"run-instances": [failure("VcpuLimitExceeded", "RunInstances")],
                              "delete-security-group": [row()], "delete-key-pair": [row()]})
        result, calls = self.host.run(responses, "up", "fixture", "t3.small", region="us-west-2")
        self.assert_nothing_left(result)
        self.assertIn("VcpuLimitExceeded", result.stderr)
        self.assertEqual(operations(calls)[-3:], ["run-instances", "delete-security-group", "delete-key-pair"])
        self.assertIn("sg-new", calls[-2]["args"])
        self.assertIn("sail-fixture", calls[-1]["args"])
        self.assertTrue(all(call["args"][2] == "us-west-2" for call in calls), calls)
        self.assertNotIn("terminate-instances", operations(calls))

    def test_refused_ingress_rule_deletes_group_and_key(self):
        responses = launch(**{"authorize-security-group-ingress": [row(err="denied\n", status=254)],
                              "delete-security-group": [row()], "delete-key-pair": [row()]})
        result, calls = self.host.run(responses, "up", "fixture", "t3.small")
        self.assert_nothing_left(result)
        self.assertEqual(operations(calls)[-2:], ["delete-security-group", "delete-key-pair"])
        self.assertNotIn("run-instances", operations(calls))

    def test_refused_group_deletes_only_the_key(self):
        responses = launch(**{"create-security-group": [failure("InvalidGroup.Duplicate", "CreateSecurityGroup")],
                              "delete-key-pair": [row()]})
        result, calls = self.host.run(responses, "up", "fixture", "t3.small")
        self.assert_nothing_left(result)
        self.assertEqual(operations(calls)[-2:], ["create-security-group", "delete-key-pair"])

    def test_refused_key_never_deletes_a_key_it_did_not_create(self):
        responses = launch(**{"create-key-pair": [failure("InvalidKeyPair.Duplicate", "CreateKeyPair")]})
        result, calls = self.host.run(responses, "up", "fixture", "t3.small")
        self.assert_nothing_left(result)
        self.assertEqual(operations(calls)[-1], "create-key-pair")

    def test_failed_cleanup_retains_state_that_down_completes(self):
        responses = launch(**{"run-instances": [failure("VcpuLimitExceeded", "RunInstances")],
                              "delete-security-group": [row(err="SSO session expired\n", status=255)]})
        result, calls = self.host.run(responses, "up", "fixture", "t3.small", region="us-west-2")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("SAIL9001", result.stderr)
        self.assertIn("down fixture", result.stderr)
        self.assertNotIn("delete-key-pair", operations(calls))
        self.assertEqual(self.host.files(), {"fixture.env": True, "fixture.pem": True, "fixture.known_hosts": False})
        self.assertEqual(self.host.env(), {"region": "us-west-2", "type": "t3.small", "arch": "amd64",
                                           "key": "sail-fixture", "sg": "sg-new"})

        result, calls = self.host.run({}, "up", "fixture", "t3.small")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("already up", result.stderr)
        self.assertEqual(calls, [])

        # A later shell without AWS_REGION still tears down in the lane's region.
        result, calls = self.host.run({"delete-security-group": [row()], "delete-key-pair": [row()]}, "down", "fixture")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(operations(calls), ["delete-security-group", "delete-key-pair"])
        self.assertTrue(all(call["args"][2] == "us-west-2" for call in calls), calls)
        self.assertFalse(any(self.host.files().values()))

    def test_instance_that_never_runs_keeps_state_and_deletes_nothing(self):
        responses = launch(**{"wait instance-running": [row(err="Waiter InstanceRunning failed\n", status=255)]})
        result, calls = self.host.run(responses, "up", "fixture", "t3.small")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("down fixture", result.stderr)
        self.assertFalse(any(op.startswith(("delete-", "terminate-")) for op in operations(calls)), calls)
        self.assertEqual(self.host.env()["id"], "i-new")

    def test_readiness_timeout_keeps_state_and_deletes_nothing(self):
        self.host.ssh_status(255)
        result, calls = self.host.run(launch(), "up", "fixture", "t3.small")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("did not become ready", result.stderr)
        self.assertIn("down fixture", result.stderr)
        self.assertFalse(any(op.startswith(("delete-", "terminate-")) for op in operations(calls)), calls)
        self.assertEqual(self.host.env()["addr"], "203.0.113.7")
        self.assertTrue(self.host.files()["fixture.pem"])


class Regions(unittest.TestCase):
    def setUp(self):
        tmp = tempfile.TemporaryDirectory(prefix="sail-perf-region-")
        self.addCleanup(tmp.cleanup)
        self.host = Host(tmp.name)

    def test_list_covers_the_default_and_every_recorded_region_once(self):
        self.host.record("region=us-west-2\nid=i-a\n", "a")
        self.host.record("region=eu-west-1\n", "b")
        self.host.record("id=i-c\nregion=us-west-2\naddr=127.0.0.1\n", "c")
        self.host.record("region=us-east-1\n", "d")
        responses = {"describe-instances": [row("d\ti-d\n"), row(), row("a\ti-a\n")]}
        result, calls = self.host.run(responses, "list")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(operations(calls), ["describe-instances"] * 3)
        self.assertEqual([call["args"][2] for call in calls], ["us-east-1", "us-west-2", "eu-west-1"])
        self.assertEqual(result.stdout, "d\ti-d\na\ti-a\n")

        result, calls = self.host.run({"describe-instances": [row(), row(), row()]}, "list", region="eu-west-1")
        self.assertEqual([call["args"][2] for call in calls], ["eu-west-1", "us-west-2", "us-east-1"])

    def test_list_without_state_uses_the_default_region(self):
        result, calls = self.host.run({"describe-instances": [row()]}, "list", region="ap-south-1")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual([call["args"][2] for call in calls], ["ap-south-1"])

    def test_list_fails_when_any_region_cannot_be_read(self):
        self.host.record("region=us-west-2\n", "a")
        responses = {"describe-instances": [row(), row(err="SSO session expired\n", status=255)]}
        result, _ = self.host.run(responses, "list")
        self.assertNotEqual(result.returncode, 0)

    def test_allow_uses_the_recorded_region_over_the_environment(self):
        self.host.record("region=us-west-2\nkey=sail-fixture\nsg=sg-owned\nid=i-owned\naddr=127.0.0.1\n")
        result, calls = self.host.run({"authorize-security-group-ingress": [row()]}, "allow", "fixture",
                                      region="eu-west-1")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual([call["args"][2] for call in calls], ["us-west-2"])
        self.assertIn("sg-owned", calls[0]["args"])

    def test_unfinished_lane_refuses_remote_commands_and_names_down(self):
        self.host.record("region=us-west-2\ntype=t3.small\narch=amd64\nkey=sail-fixture\n")
        for command in (("run", "fixture", "true"), ("ssh", "fixture"), ("sync", "fixture"),
                        ("extend", "fixture", "60")):
            with self.subTest(command=command):
                result, calls = self.host.run({}, *command)
                self.assertNotEqual(result.returncode, 0)
                self.assertIn("down fixture", result.stderr)
                self.assertEqual(calls, [])


if __name__ == "__main__":
    unittest.main()
