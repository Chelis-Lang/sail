#!/usr/bin/env python3
"""Exercise Linux host teardown against a deterministic fake AWS CLI."""
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest


class Teardown(unittest.TestCase):
    def invoke(self, responses, recorded=True):
        with tempfile.TemporaryDirectory(prefix="sail-perf-down-") as tmp:
            root = Path(tmp)
            state = root / "state"
            state.mkdir()
            names = ["fixture.env", "fixture.pem", "fixture.known_hosts"]
            if recorded:
                (state / names[0]).write_text("id=i-owned\nsg=sg-owned\nkey=sail-owned\naddr=127.0.0.1\nregion=us-west-2\n")
                for name in names[1:]:
                    (state / name).write_text("owned fixture\n")
            fixture = root / "responses.json"
            fixture.write_text(json.dumps(responses))
            calls = root / "calls.jsonl"
            aws = root / "aws"
            aws.write_text('''#!/usr/bin/env python3
import json,os,pathlib,sys
args=sys.argv[1:]
operation=args[3] if len(args)>3 and args[:2]==['ec2','--region'] else 'unexpected'
if operation=='wait':operation+=' '+args[4]
with open(os.environ['SAIL_FAKE_AWS_CALLS'],'a') as f:f.write(json.dumps(args)+'\\n')
p=pathlib.Path(os.environ['SAIL_FAKE_AWS_RESPONSES']);responses=json.loads(p.read_text())
rows=responses.get(operation,[])
if not rows:
 print('Unexpected fake AWS command: '+operation,file=sys.stderr);sys.exit(99)
row=rows.pop(0);p.write_text(json.dumps(responses))
print(row.get('out',''),end='')
print(row.get('err',''),end='',file=sys.stderr)
sys.exit(row.get('status',0))
''')
            aws.chmod(0o755)
            env = dict(os.environ)
            env.update(PATH=f"{root}:{env['PATH']}", SAIL_PERF_STATE_DIR=str(state),
                       SAIL_FAKE_AWS_CALLS=str(calls), SAIL_FAKE_AWS_RESPONSES=str(fixture))
            script = os.environ.get("SAIL_PERF_HOST_SCRIPT", str(Path(__file__).with_name("perf-host.sh")))
            result = subprocess.run(["bash", script, "down", "fixture"], env=env,
                                    text=True, capture_output=True, timeout=10)
            observed = [json.loads(line) for line in calls.read_text().splitlines()] if calls.exists() else []
            return result, observed, {name: (state / name).exists() for name in names}

    @staticmethod
    def row(out="", err="", status=0):
        return {"out": out, "err": err, "status": status}

    def state(self, name):
        return self.row(json.dumps({"Reservations": [{"Instances": [
            {"InstanceId": "i-owned", "State": {"Name": name}}
        ]}]}))

    def assert_retained(self, result, calls, files):
        self.assertNotEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertTrue(all(files.values()), files)
        self.assertFalse(any(call[3].startswith("delete-") for call in calls), calls)

    def test_expired_sso_never_deletes_state_or_resources(self):
        responses = {operation: [self.row(err="SSO session expired\n", status=255)]
                     for operation in ("describe-instances", "terminate-instances",
                                       "wait instance-terminated", "delete-security-group", "delete-key-pair")}
        result = self.invoke(responses)
        self.assert_retained(*result)
        self.assertEqual(len(result[1]), 1)

    def test_termination_failure_retains_everything(self):
        self.assert_retained(*self.invoke({"describe-instances": [self.state("running")],
                                          "terminate-instances": [self.row(err="denied\n", status=255)]}))

    def test_wait_failure_retains_everything(self):
        self.assert_retained(*self.invoke({"describe-instances": [self.state("running")],
                                          "terminate-instances": [self.row()],
                                          "wait instance-terminated": [self.row(err="wait expired\n", status=255)]}))

    def test_successful_wait_still_requires_confirmed_termination(self):
        self.assert_retained(*self.invoke({"describe-instances": [self.state("running"), self.state("running")],
                                          "terminate-instances": [self.row()],
                                          "wait instance-terminated": [self.row()]}))

    def test_confirmation_api_failure_retains_everything(self):
        self.assert_retained(*self.invoke({"describe-instances": [self.state("running"), self.row(err="SSO expired\n", status=255)],
                                          "terminate-instances": [self.row()],
                                          "wait instance-terminated": [self.row()]}))

    def test_unknown_instance_state_refuses(self):
        self.assert_retained(*self.invoke({"describe-instances": [self.row("None\n")]}))

    def test_not_found_text_in_another_error_is_not_absence(self):
        error = "SSO failed; previous error was An error occurred (InvalidInstanceID.NotFound) when calling the DescribeInstances operation: missing\n"
        self.assert_retained(*self.invoke({"describe-instances": [self.row(err=error, status=255)]}))

    def test_running_instance_is_verified_before_resource_deletion(self):
        result, calls, files = self.invoke({"describe-instances": [self.state("running"), self.state("terminated")],
                                           "terminate-instances": [self.row()],
                                           "wait instance-terminated": [self.row()],
                                           "delete-security-group": [self.row()], "delete-key-pair": [self.row()]})
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertFalse(any(files.values()))
        self.assertEqual([c[3] for c in calls], ["describe-instances", "terminate-instances", "wait", "describe-instances", "delete-security-group", "delete-key-pair"])
        self.assertTrue(all(c[2] == "us-west-2" for c in calls), calls)
        self.assertIn("i-owned", calls[1])
        self.assertIn("sg-owned", calls[-2])
        self.assertIn("sail-owned", calls[-1])

    def test_already_terminated_instance_skips_termination(self):
        result, calls, files = self.invoke({"describe-instances": [self.state("terminated")],
                                           "delete-security-group": [self.row()], "delete-key-pair": [self.row()]})
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertFalse(any(files.values()))
        self.assertFalse(any(c[3] in ("terminate-instances", "wait") for c in calls))

    def test_successful_empty_reservations_is_absence(self):
        result, calls, files = self.invoke({"describe-instances": [self.row(json.dumps({"Reservations": []}))],
                                           "delete-security-group": [self.row()], "delete-key-pair": [self.row()]})
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertFalse(any(files.values()))
        self.assertEqual([c[3] for c in calls], ["describe-instances", "delete-security-group", "delete-key-pair"])

    def test_ambiguous_json_or_mismatched_instance_refuses(self):
        for response in ({}, {"Reservations": None}, {"Reservations": {}},
                         {"Reservations": [{}]}, {"Reservations": [{"Instances": []}]},
                         {"Reservations": [{"Instances": [{"InstanceId": "i-other", "State": {"Name": "terminated"}}]}]},
                         {"Reservations": [{"Instances": [{"InstanceId": "i-owned", "State": {"Name": "unknown"}}]}]}):
            with self.subTest(response=response):
                self.assert_retained(*self.invoke({"describe-instances": [self.row(json.dumps(response))]}))

    def test_already_absent_resources_are_idempotent(self):
        def missing(code, operation):
            return self.row(err=f"\nAn error occurred ({code}) when calling the {operation} operation: absent\n", status=255)
        result, calls, files = self.invoke({"describe-instances": [missing("InvalidInstanceID.NotFound", "DescribeInstances")],
                                           "delete-security-group": [missing("InvalidGroup.NotFound", "DeleteSecurityGroup")],
                                           "delete-key-pair": [missing("InvalidKeyPair.NotFound", "DeleteKeyPair")]})
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertFalse(any(files.values()))
        self.assertEqual(len(calls), 3)
        result, calls, files = self.invoke({}, recorded=False)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(calls, [])
        self.assertFalse(any(files.values()))

    def test_security_group_error_keeps_key_and_retry_state(self):
        result, calls, files = self.invoke({"describe-instances": [self.state("terminated")],
                                           "delete-security-group": [self.row(err="dependency violation\n", status=255)]})
        self.assertNotEqual(result.returncode, 0)
        self.assertTrue(all(files.values()))
        self.assertFalse(any(c[3] == "delete-key-pair" for c in calls))

    def test_key_deletion_error_keeps_retry_state(self):
        result, _, files = self.invoke({"describe-instances": [self.state("terminated")],
                                       "delete-security-group": [self.row()],
                                       "delete-key-pair": [self.row(err="access denied\n", status=255)]})
        self.assertNotEqual(result.returncode, 0)
        self.assertTrue(all(files.values()))


if __name__ == "__main__":
    unittest.main()
