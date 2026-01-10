#!/bin/bash

# Test installation scripts on Alpine Linux using OpenRC

set -e

docker build -f scripts/tests/Containerfile.systemd -t shuthost-test-systemd .

docker build -f scripts/tests/Containerfile.alpine -t shuthost-test-alpine .

alpine_test() {
    docker run --rm -t shuthost-test-alpine "$1"
}

systemd_test() {
    docker run --rm -t --privileged shuthost-test-systemd "$1"
}

set +e

pids=()
logs=()

for init in alpine systemd; do
  for embed in "target/" ""; do
    for script in coordinator host_agent; do
      path="./${embed}scripts/enduser_installers/${script}.sh"
      log_name="${init}_${embed%/}_${script}"
      log_file="/tmp/${log_name}.log"
      if [ "$init" = "alpine" ]; then
        alpine_test "$path" > "$log_file" 2>&1 &
      else
        systemd_test "$path" > "$log_file" 2>&1 &
      fi
      pids+=($!)
      logs+=("$log_file")
    done
  done
done

failed=false
failed_tests=()

for i in "${!pids[@]}"; do
  if ! wait "${pids[$i]}"; then
    failed=true
    failed_tests+=("${logs[$i]}")
  fi
done

if $failed; then
  echo "Failed tests: ${failed_tests[*]}"
  exit 1
fi
