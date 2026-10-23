#!/usr/bin/env bash
# Ephemeral read-only Git authentication for the pinned private dependencies.
# The helper reads a secret from the environment only when Git asks for it.
set -euo pipefail
if [ "$#" -eq 0 ]; then
  echo 'error[SAIL4213]: expected a command to run' >&2
  exit 2
fi
if [ -n "${SAIL_DEPENDENCY_READ_TOKEN:-}" ]; then
  sail_git_count="${GIT_CONFIG_COUNT:-0}"
  if ! [[ "$sail_git_count" =~ ^[0-9]+$ ]] || [ "$sail_git_count" -gt 100 ]; then
    echo 'error[SAIL4213]: invalid Git environment configuration count' >&2
    exit 2
  fi
  export CARGO_NET_GIT_FETCH_WITH_CLI=true
  export SAIL_PRIVATE_GIT_READY=1
  # Clear other helpers for this command, without writing any Git configuration.
  export "GIT_CONFIG_KEY_${sail_git_count}=credential.helper"
  export "GIT_CONFIG_VALUE_${sail_git_count}="
  sail_git_count=$((sail_git_count + 1))
  export "GIT_CONFIG_KEY_${sail_git_count}=credential.https://github.com.useHttpPath"
  export "GIT_CONFIG_VALUE_${sail_git_count}=true"
  sail_git_count=$((sail_git_count + 1))
  export "GIT_CONFIG_KEY_${sail_git_count}=credential.https://github.com.helper"
  export "GIT_CONFIG_VALUE_${sail_git_count}=!f() {
    [ \"\$1\" = get ] || return 0
    sail_protocol= sail_host= sail_path=
    while IFS= read -r sail_field; do
      [ -n \"\$sail_field\" ] || break
      case \"\$sail_field\" in
        protocol=*) sail_protocol=\${sail_field#protocol=} ;;
        host=*) sail_host=\${sail_field#host=} ;;
        path=*) sail_path=\${sail_field#path=} ;;
      esac
    done
    case \"\$sail_protocol:\$sail_host:\$sail_path\" in
      https:github.com:Chelis-Lang/sail|https:github.com:Chelis-Lang/sail.git|https:github.com:Chelis-Lang/dive|https:github.com:Chelis-Lang/dive.git)
        printf 'username=x-access-token\\npassword=%s\\n' \"\$SAIL_DEPENDENCY_READ_TOKEN\" ;;
    esac
  }; f"
  export GIT_CONFIG_COUNT=$((sail_git_count + 1))
fi
exec "$@"
