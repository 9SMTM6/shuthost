#!/sbin/openrc-run
# shellcheck disable=SC1008,SC2034

supervisor=supervise-daemon
name="{ name }"
description="{ description }"
command="/usr/local/sbin/{ name }"
command_args="service --port={ port } --broadcast-port={ broadcast_port } --shutdown-command=\"{ shutdown_command }\" --hostname={ hostname } --init-system openrc"
command_user="root"
pidfile="/run/${RC_SVCNAME}.pid"

export SHUTHOST_SHARED_SECRET="{ secret }"

depend() {
    need net
}
