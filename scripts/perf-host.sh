#!/usr/bin/env bash
# Native x86-64 Linux EC2 hosts for builds, gates and benchmarks (ADR 0038).
#
#   scripts/perf-host.sh up LANE [INSTANCE_TYPE]   launch (default c7i.4xlarge)
#   scripts/perf-host.sh sync LANE                 rsync this checkout to ~/sail
#   scripts/perf-host.sh run LANE CMD...           run CMD in ~/sail
#   scripts/perf-host.sh ssh LANE [CMD...]         plain ssh
#   scripts/perf-host.sh extend LANE MINUTES       reset the self-termination watchdog
#   scripts/perf-host.sh allow [LANE]              allow SSH from this machine's current IP
#                                                  (every lane by default; after a network change)
#   scripts/perf-host.sh down LANE                 verify termination, then delete group/key/state
#   scripts/perf-host.sh list                      every running Project=sail instance
#
# Every host terminates itself after SAIL_PERF_TTL_MIN minutes (default 480)
# unless extended, so a forgotten host cannot leak. Resources are tagged
# Project=sail and Lane=LANE. State lives in ~/.sail-perf/LANE.*
# (SAIL_PERF_STATE_DIR overrides that directory). Failed teardown retains it.
set -euo pipefail

region="${AWS_REGION:-us-east-1}"
state_dir="${SAIL_PERF_STATE_DIR:-${HOME}/.sail-perf}"
ttl="${SAIL_PERF_TTL_MIN:-480}"
aws_ec2() { aws ec2 --region "$region" "$@"; }

usage() { sed -n '2,15p' "$0" | sed 's/^# \{0,1\}//'; exit 2; }

require_lane() {
  [ -n "${1:-}" ] || usage
  case "$1" in *[!a-z0-9-]*) echo "lane must be [a-z0-9-]" >&2; exit 2 ;; esac
}

host_of() {
  local f="$state_dir/$1.env"
  [ -f "$f" ] || { echo "no host for lane $1 (run: $0 up $1)" >&2; exit 1; }
  # shellcheck disable=SC1090
  . "$f"
}

ssh_opts() {
  echo -i "$state_dir/$1.pem" -o StrictHostKeyChecking=accept-new \
    -o UserKnownHostsFile="$state_dir/$1.known_hosts" -o ServerAliveInterval=30 \
    -o ConnectTimeout=10
}

cmd_up() {
  require_lane "${1:-}"
  local lane="$1" type="${2:-c7i.4xlarge}" name="sail-$1"
  mkdir -p "$state_dir" && chmod 700 "$state_dir"
  if [ -f "$state_dir/$lane.env" ]; then echo "lane $lane already up" >&2; exit 1; fi

  local ami vpc ip sg id
  ami=$(aws_ec2 describe-images --owners 099720109477 \
    --filters 'Name=name,Values=ubuntu/images/hvm-ssd-gp3/ubuntu-noble-24.04-amd64-server-*' \
    --query 'sort_by(Images,&CreationDate)[-1].ImageId' --output text)
  vpc=$(aws_ec2 describe-vpcs --filters Name=isDefault,Values=true --query 'Vpcs[0].VpcId' --output text)
  ip=$(curl -fsS https://checkip.amazonaws.com)

  aws_ec2 create-key-pair --key-name "$name" --key-type ed25519 \
    --tag-specifications "ResourceType=key-pair,Tags=[{Key=Project,Value=sail},{Key=Lane,Value=$lane}]" \
    --query KeyMaterial --output text >"$state_dir/$lane.pem"
  chmod 600 "$state_dir/$lane.pem"
  sg=$(aws_ec2 create-security-group --group-name "$name" --description "sail perf host $lane" \
    --vpc-id "$vpc" --tag-specifications "ResourceType=security-group,Tags=[{Key=Project,Value=sail},{Key=Lane,Value=$lane}]" \
    --query GroupId --output text)
  aws_ec2 authorize-security-group-ingress --group-id "$sg" --protocol tcp --port 22 --cidr "$ip/32" >/dev/null

  local user_data
  user_data=$(cat <<EOF
#!/bin/bash
shutdown -h +$ttl
export DEBIAN_FRONTEND=noninteractive
apt-get update -q
apt-get install -yq docker.io docker-buildx git rsync jq build-essential pkg-config libssl-dev python3 curl
usermod -aG docker ubuntu
touch /var/lib/sail-perf-ready
EOF
)
  id=$(aws_ec2 run-instances --image-id "$ami" --instance-type "$type" --key-name "$name" \
    --security-group-ids "$sg" --instance-initiated-shutdown-behavior terminate \
    --block-device-mappings 'DeviceName=/dev/sda1,Ebs={VolumeSize=200,VolumeType=gp3,Iops=6000,Throughput=500,DeleteOnTermination=true}' \
    --tag-specifications "ResourceType=instance,Tags=[{Key=Name,Value=$name},{Key=Project,Value=sail},{Key=Lane,Value=$lane}]" \
    --user-data "$user_data" --query 'Instances[0].InstanceId' --output text)
  printf 'id=%s\nsg=%s\nkey=%s\ntype=%s\nregion=%s\n' "$id" "$sg" "$name" "$type" "$region" >"$state_dir/$lane.env"
  aws_ec2 wait instance-running --instance-ids "$id"
  local addr
  addr=$(aws_ec2 describe-instances --instance-ids "$id" \
    --query 'Reservations[0].Instances[0].PublicIpAddress' --output text)
  echo "addr=$addr" >>"$state_dir/$lane.env"
  echo "waiting for $lane ($id, $type, $addr) to finish provisioning..." >&2
  local i
  for i in $(seq 1 90); do
    # shellcheck disable=SC2046
    if ssh $(ssh_opts "$lane") "ubuntu@$addr" test -f /var/lib/sail-perf-ready 2>/dev/null; then
      echo "$lane ready: $id $addr (self-terminates in ${ttl} min)"; return 0
    fi
    sleep 10
  done
  echo "$lane did not become ready; run: $0 down $lane" >&2; exit 1
}

cmd_sync() {
  require_lane "${1:-}"; host_of "$1"
  # The helper freezes actual working bytes, preserves staged objects, and
  # builds standalone metadata instead of copying a linked .git pointer.
  python3 "$(dirname "${BASH_SOURCE[0]}")/sync-native-checkout.py" \
    --source "$PWD" --destination "ubuntu@$addr" \
    --ssh-command "ssh $(ssh_opts "$1")"
}

cmd_run() {
  require_lane "${1:-}"; local lane="$1"; shift; host_of "$lane"
  local remote
  remote="cd ~/sail && $(printf '%q ' "$@")"
  # shellcheck disable=SC2046
  ssh $(ssh_opts "$lane") "ubuntu@$addr" "bash -lc $(printf '%q' "$remote")" </dev/null
}

cmd_ssh() {
  require_lane "${1:-}"; local lane="$1"; shift; host_of "$lane"
  # shellcheck disable=SC2046
  exec ssh $(ssh_opts "$lane") "ubuntu@$addr" "$@"
}

cmd_extend() {
  require_lane "${1:-}"; host_of "$1"
  # shellcheck disable=SC2046
  ssh $(ssh_opts "$1") "ubuntu@$addr" "sudo shutdown -c; sudo shutdown -h +${2:?minutes}"
}

cmd_allow() {
  local ip lanes lane
  ip=$(curl -fsS https://checkip.amazonaws.com)
  if [ -n "${1:-}" ]; then require_lane "$1"; lanes=$1
  else lanes=$(cd "$state_dir" 2>/dev/null && ls -- *.env 2>/dev/null | sed 's/\.env$//'); fi
  for lane in $lanes; do
    ( host_of "$lane"
      aws_ec2 authorize-security-group-ingress --group-id "$sg" --protocol tcp --port 22 \
        --cidr "$ip/32" >/dev/null 2>&1 || true
      echo "$lane allows $ip" )
  done
}

# Accept only the expected AWS operation/code, never arbitrary "not found" text.
# The CLI can put blank lines before its error message.
ec2_absent() {
  local message="$1" code="$2" operation="$3"
  while [ "${message#$'\n'}" != "$message" ]; do message="${message#$'\n'}"; done
  case "$message" in
    "An error occurred ($code) when calling the $operation operation: "*) return 0 ;;
    *) return 1 ;;
  esac
}

down_failed() {
  printf 'error[SAIL9001]: %s; retain lane state and retry down after fixing the error\n' "$1" >&2
  [ -z "${2:-}" ] || printf '%s\n' "$2" >&2
}

down_instance_state() {
  local result state
  if result=$(aws_ec2 describe-instances --instance-ids "$1" --output json 2>&1); then
    # The API can return success with an empty Reservations array for an old
    # terminated ID. A missing/malformed response or mismatched ID is ambiguous.
    if state=$(printf '%s\n' "$result" | python3 -c '
import json, sys
try:
    response = json.load(sys.stdin)
    reservations = response["Reservations"]
    if not isinstance(reservations, list):
        raise ValueError("Reservations is not an array")
    if not reservations:
        print("absent")
    else:
        if len(reservations) != 1:
            raise ValueError("ambiguous reservations")
        instances = reservations[0]["Instances"]
        if not isinstance(instances, list) or len(instances) != 1:
            raise ValueError("ambiguous instances")
        instance = instances[0]
        state = instance["State"]["Name"]
        if instance["InstanceId"] != sys.argv[1] or state not in (
            "pending", "running", "stopping", "stopped", "shutting-down", "terminated"
        ):
            raise ValueError("unknown identity or state")
        print(state)
except (KeyError, TypeError, ValueError) as error:
    print("invalid instance response: " + str(error), file=sys.stderr)
    sys.exit(1)
' "$1"); then
      printf '%s\n' "$state"
    else
      down_failed "AWS returned an unknown state for $1"; return 1
    fi
  elif ec2_absent "$result" InvalidInstanceID.NotFound DescribeInstances; then
    printf 'absent\n'
  else
    down_failed "cannot establish the state of $1" "$result"; return 1
  fi
}

cmd_down() {
  require_lane "${1:-}"; local lane="$1" state result
  if [ ! -f "$state_dir/$lane.env" ]; then
    echo "no recorded host for lane $lane"; return 0
  fi
  host_of "$lane"
  state=$(down_instance_state "$id") || return 1
  case "$state" in
    terminated | absent) ;;
    *)
      if ! result=$(aws_ec2 terminate-instances --instance-ids "$id" 2>&1); then
        down_failed "cannot terminate $lane ($id)" "$result"; return 1
      fi
      if ! result=$(aws_ec2 wait instance-terminated --instance-ids "$id" 2>&1); then
        down_failed "termination wait failed for $lane ($id)" "$result"; return 1
      fi
      state=$(down_instance_state "$id") || return 1
      case "$state" in
        terminated | absent) ;;
        *) down_failed "termination is unconfirmed for $lane ($id): $state"; return 1 ;;
      esac ;;
  esac
  if result=$(aws_ec2 delete-security-group --group-id "$sg" 2>&1); then :
  elif ! ec2_absent "$result" InvalidGroup.NotFound DeleteSecurityGroup; then
    down_failed "cannot delete security group $sg" "$result"; return 1
  fi
  if result=$(aws_ec2 delete-key-pair --key-name "$key" 2>&1); then :
  elif ! ec2_absent "$result" InvalidKeyPair.NotFound DeleteKeyPair; then
    down_failed "cannot delete key pair $key" "$result"; return 1
  fi
  if ! rm -f "$state_dir/$lane".{env,pem,known_hosts}; then
    down_failed "resources are gone but local state cleanup failed for $lane"; return 1
  fi
  echo "$lane down ($id)"
}

cmd_list() {
  aws_ec2 describe-instances \
    --filters Name=tag:Project,Values=sail Name=instance-state-name,Values=pending,running,stopping,stopped \
    --query 'Reservations[].Instances[].[Tags[?Key==`Lane`]|[0].Value,InstanceId,InstanceType,State.Name,PublicIpAddress,LaunchTime]' \
    --output text
}

sub="${1:-}"; shift || true
case "$sub" in
  up) cmd_up "$@" ;; sync) cmd_sync "$@" ;; run) cmd_run "$@" ;; ssh) cmd_ssh "$@" ;;
  extend) cmd_extend "$@" ;; allow) cmd_allow "$@" ;; down) cmd_down "$@" ;; list) cmd_list ;;
  *) usage ;;
esac
