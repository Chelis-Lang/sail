#!/usr/bin/env bash
# EC2 Mac (Apple silicon) hosts for clean macOS measurements (ADR 0038).
#
#   scripts/perf-host-mac.sh up LANE [AZ]        allocate a mac-m4.metal dedicated host,
#                                                schedule its release, launch macOS on it
#   scripts/perf-host-mac.sh sync LANE           rsync this checkout to ~/sail
#   scripts/perf-host-mac.sh run LANE CMD...     run CMD in ~/sail
#   scripts/perf-host-mac.sh ssh LANE [CMD...]   plain ssh
#   scripts/perf-host-mac.sh down LANE           terminate; delete key and security group
#   scripts/perf-host-mac.sh status LANE         host, instance and release schedule
#
# Apple's licence makes a Mac dedicated host billable for at least 24 hours,
# and AWS refuses to release it earlier. `up` therefore schedules the release
# on the AWS side, so it happens even if this machine is off: an EventBridge
# Scheduler one-time schedule at allocation + 24 h 10 min calls
# ec2:ReleaseHosts through the role sail-LANE-release, which may release
# only that host, and deletes itself after it runs. `down` terminates the
# instance at once; the host, its schedule and the role stay until the
# release fires, and `down` prints the role's cleanup commands for after that.
# Everything is tagged Project=sail, Lane=LANE. State lives in ~/.sail-perf/LANE.*.
set -euo pipefail

region="${AWS_REGION:-us-east-1}"
state_dir="${HOME}/.sail-perf"
type="${SAIL_MAC_INSTANCE_TYPE:-mac-m4.metal}"
aws_ec2() { aws ec2 --region "$region" "$@"; }

usage() { sed -n '2,20p' "$0" | sed 's/^# \{0,1\}//'; exit 2; }

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

tags() { echo "{Key=Project,Value=sail},{Key=Lane,Value=$1}"; }

schedule_release() {
  local lane="$1" host="$2" allocated="$3" account role at policy
  account=$(aws sts get-caller-identity --query Account --output text)
  role="sail-$lane-release"
  if ! aws iam get-role --role-name "$role" >/dev/null 2>&1; then
    aws iam create-role --role-name "$role" \
      --tags Key=Project,Value=sail "Key=Lane,Value=$lane" \
      --assume-role-policy-document "$(printf '{"Version":"2012-10-17","Statement":[{"Effect":"Allow","Principal":{"Service":"scheduler.amazonaws.com"},"Action":"sts:AssumeRole","Condition":{"StringEquals":{"aws:SourceAccount":"%s"}}}]}' "$account")" >/dev/null
  fi
  policy=$(printf '{"Version":"2012-10-17","Statement":[{"Effect":"Allow","Action":"ec2:ReleaseHosts","Resource":"arn:aws:ec2:%s:%s:dedicated-host/%s"}]}' "$region" "$account" "$host")
  aws iam put-role-policy --role-name "$role" --policy-name release-host --policy-document "$policy"
  sleep 10 # IAM propagation before the scheduler validates the role
  at=$(python3 -c 'import datetime,sys; t=datetime.datetime.fromisoformat(sys.argv[1].replace("Z","+00:00")); print((t+datetime.timedelta(hours=24,minutes=10)).strftime("%Y-%m-%dT%H:%M:%S"))' "$allocated")
  aws scheduler create-schedule-group --region "$region" --name "sail-$lane" \
    --tags Key=Project,Value=sail "Key=Lane,Value=$lane" >/dev/null 2>&1 || true
  aws scheduler create-schedule --region "$region" --group-name "sail-$lane" \
    --name "sail-$lane-release-$host" --schedule-expression "at($at)" \
    --schedule-expression-timezone UTC --flexible-time-window Mode=OFF \
    --action-after-completion DELETE \
    --target "$(printf '{"Arn":"arn:aws:scheduler:::aws-sdk:ec2:releaseHosts","RoleArn":"arn:aws:iam::%s:role/%s","Input":"{\\"HostIds\\":[\\"%s\\"]}","RetryPolicy":{"MaximumRetryAttempts":185,"MaximumEventAgeInSeconds":86400}}' "$account" "$role" "$host")" >/dev/null
  printf 'role=%s\nschedule_group=%s\nschedule=%s\nrelease_at=%sZ\n' \
    "$role" "sail-$lane" "sail-$lane-release-$host" "$at" >>"$state_dir/$lane.env"
  aws scheduler get-schedule --region "$region" --group-name "sail-$lane" \
    --name "sail-$lane-release-$host" --query '[Name,ScheduleExpression,State,ActionAfterCompletion]' --output text
}

cmd_up() {
  require_lane "${1:-}"
  local lane="$1" want_az="${2:-}" name="sail-$1"
  mkdir -p "$state_dir" && chmod 700 "$state_dir"
  if [ -f "$state_dir/$lane.env" ]; then . "$state_dir/$lane.env"; fi
  if [ -z "${host:-}" ]; then
    local az azs
    azs=${want_az:-$(aws_ec2 describe-instance-type-offerings --location-type availability-zone \
      --filters "Name=instance-type,Values=$type" --query 'InstanceTypeOfferings[].Location' --output text)}
    for az in $azs; do
      host=$(aws_ec2 allocate-hosts --instance-type "$type" --availability-zone "$az" --quantity 1 \
        --tag-specifications "ResourceType=dedicated-host,Tags=[$(tags "$lane")]" \
        --query 'HostIds[0]' --output text 2>/dev/null) && [ -n "$host" ] && break
      echo "no $type capacity in $az" >&2; host=
    done
    [ -n "$host" ] || { echo "could not allocate a $type host" >&2; exit 1; }
    local allocated
    allocated=$(aws_ec2 describe-hosts --host-ids "$host" --query 'Hosts[0].AllocationTime' --output text)
    printf 'host=%s\naz=%s\nallocated=%s\n' "$host" "$az" "$allocated" >"$state_dir/$lane.env"
    echo "allocated $host in $az at $allocated" >&2
    schedule_release "$lane" "$host" "$allocated"
    . "$state_dir/$lane.env"
  fi
  if [ -n "${id:-}" ]; then echo "lane $lane already has instance $id" >&2; exit 1; fi

  local ami vpc ip subnet
  ami=${SAIL_MAC_AMI:-$(aws_ec2 describe-images --owners amazon \
    --filters 'Name=name,Values=amzn-ec2-macos-*-arm64' 'Name=state,Values=available' \
    --query 'sort_by(Images,&CreationDate)[-1].ImageId' --output text)}
  vpc=$(aws_ec2 describe-vpcs --filters Name=isDefault,Values=true --query 'Vpcs[0].VpcId' --output text)
  subnet=$(aws_ec2 describe-subnets --filters "Name=vpc-id,Values=$vpc" "Name=availability-zone,Values=$az" \
    "Name=default-for-az,Values=true" --query 'Subnets[0].SubnetId' --output text)
  ip=$(curl -fsS https://checkip.amazonaws.com)
  aws_ec2 create-key-pair --key-name "$name" --key-type ed25519 \
    --tag-specifications "ResourceType=key-pair,Tags=[$(tags "$lane")]" \
    --query KeyMaterial --output text >"$state_dir/$lane.pem"
  chmod 600 "$state_dir/$lane.pem"
  sg=$(aws_ec2 create-security-group --group-name "$name" --description "sail mac host $lane" \
    --vpc-id "$vpc" --tag-specifications "ResourceType=security-group,Tags=[$(tags "$lane")]" \
    --query GroupId --output text)
  aws_ec2 authorize-security-group-ingress --group-id "$sg" --protocol tcp --port 22 --cidr "$ip/32" >/dev/null
  id=$(aws_ec2 run-instances --image-id "$ami" --instance-type "$type" --key-name "$name" \
    --placement "HostId=$host,Tenancy=host" --subnet-id "$subnet" --security-group-ids "$sg" \
    --block-device-mappings 'DeviceName=/dev/sda1,Ebs={VolumeSize=200,VolumeType=gp3,Iops=6000,Throughput=500,DeleteOnTermination=true}' \
    --tag-specifications "ResourceType=instance,Tags=[{Key=Name,Value=$name},$(tags "$lane")]" \
    --query 'Instances[0].InstanceId' --output text)
  printf 'id=%s\nsg=%s\nkey=%s\nami=%s\n' "$id" "$sg" "$name" "$ami" >>"$state_dir/$lane.env"
  aws_ec2 wait instance-running --instance-ids "$id"
  addr=$(aws_ec2 describe-instances --instance-ids "$id" \
    --query 'Reservations[0].Instances[0].PublicIpAddress' --output text)
  echo "addr=$addr" >>"$state_dir/$lane.env"
  echo "waiting for $lane ($id on $host, $addr); macOS takes 10-20 minutes..." >&2
  local i
  for i in $(seq 1 180); do
    # shellcheck disable=SC2046
    if ssh $(ssh_opts "$lane") "ec2-user@$addr" true 2>/dev/null </dev/null; then
      # Grow APFS into the 200 GB volume (a no-op once done).
      # shellcheck disable=SC2046
      ssh $(ssh_opts "$lane") "ec2-user@$addr" \
        'PDISK=$(diskutil list physical external | head -n1 | cut -d" " -f1); APFSCONT=$(diskutil list physical external | grep Apple_APFS | tr -s " " | cut -d" " -f8); yes | sudo diskutil repairDisk $PDISK >/dev/null 2>&1; sudo diskutil apfs resizeContainer $APFSCONT 0 >/dev/null 2>&1; df -h / | tail -1' </dev/null || true
      echo "$lane ready: $id $addr on $host (release scheduled for $release_at)"; return 0
    fi
    sleep 10
  done
  echo "$lane did not become reachable; run: $0 down $lane" >&2; exit 1
}

cmd_sync() {
  require_lane "${1:-}"; host_of "$1"
  # shellcheck disable=SC2046
  rsync -az --delete -e "ssh $(ssh_opts "$1")" \
    --exclude target/ --exclude .claude/ --exclude '__pycache__/' --exclude conformance-out/ \
    --exclude /dist/ --exclude /release-out/ \
    "$(git rev-parse --show-toplevel)/" "ec2-user@$addr:sail/"
}

cmd_run() {
  require_lane "${1:-}"; local lane="$1"; shift; host_of "$lane"
  local remote
  remote="cd ~/sail && $(printf '%q ' "$@")"
  # shellcheck disable=SC2046
  ssh $(ssh_opts "$lane") "ec2-user@$addr" "bash -lc $(printf '%q' "$remote")" </dev/null
}

cmd_ssh() {
  require_lane "${1:-}"; local lane="$1"; shift; host_of "$lane"
  # shellcheck disable=SC2046
  exec ssh $(ssh_opts "$lane") "ec2-user@$addr" "$@"
}

cmd_down() {
  require_lane "${1:-}"; local lane="$1"; host_of "$lane"
  if [ -n "${id:-}" ]; then
    aws_ec2 terminate-instances --instance-ids "$id" >/dev/null || true
    echo "$lane: termination of $id requested at $(date -u +%Y-%m-%dT%H:%M:%SZ); a Mac can take 30 minutes or more" >&2
    local i state=
    for i in $(seq 1 120); do # up to an hour
      state=$(aws_ec2 describe-instances --instance-ids "$id" \
        --query 'Reservations[0].Instances[0].State.Name' --output text 2>/dev/null || true)
      [ "$state" = terminated ] && break
      sleep 30
    done
    if [ "$state" = terminated ]; then
      echo "$lane: instance $id terminated by $(date -u +%Y-%m-%dT%H:%M:%SZ)"; id=
    else
      echo "$lane: instance $id is still $state; run: $0 down $lane again" >&2
    fi
  fi
  [ -z "${key:-}" ] || { aws_ec2 delete-key-pair --key-name "$key" >/dev/null && key=; } || true
  [ -z "${sg:-}" ] || [ -n "${id:-}" ] || { aws_ec2 delete-security-group --group-id "$sg" >/dev/null && sg=; } || true
  [ -n "${id:-}" ] || rm -f "$state_dir/$lane".{pem,known_hosts}
  # Keep the host record (and anything not yet deleted): the host stays
  # allocated until the scheduled release.
  printf 'host=%s\naz=%s\nallocated=%s\nrole=%s\nschedule_group=%s\nschedule=%s\nrelease_at=%s\nid=%s\nsg=%s\nkey=%s\naddr=%s\n' \
    "$host" "$az" "$allocated" "$role" "$schedule_group" "$schedule" "$release_at" \
    "${id:-}" "${sg:-}" "${key:-}" "${addr:-}" >"$state_dir/$lane.env"
  cat <<EOF
$lane: host $host stays allocated until its scheduled release at $release_at.
After that, check and clean up:
  aws ec2 describe-hosts --region $region --host-ids $host --query 'Hosts[0].State'   # released
  aws iam delete-role-policy --role-name $role --policy-name release-host
  aws iam delete-role --role-name $role
  aws scheduler delete-schedule-group --region $region --name $schedule_group
  rm -f $state_dir/$lane.env
EOF
}

cmd_status() {
  require_lane "${1:-}"; host_of "$1"
  aws_ec2 describe-hosts --host-ids "$host" \
    --query 'Hosts[0].[HostId,State,AvailabilityZone,AllocationTime,Instances[0].InstanceId]' --output text
  aws scheduler get-schedule --region "$region" --group-name "$schedule_group" --name "$schedule" \
    --query '[Name,ScheduleExpression,State]' --output text 2>/dev/null || echo "schedule $schedule: gone (fired or deleted)"
}

sub="${1:-}"; shift || true
case "$sub" in
  up) cmd_up "$@" ;; sync) cmd_sync "$@" ;; run) cmd_run "$@" ;; ssh) cmd_ssh "$@" ;;
  down) cmd_down "$@" ;; status) cmd_status "$@" ;;
  *) usage ;;
esac
