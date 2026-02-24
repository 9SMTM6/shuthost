#!/sbin/openrc-run
# shellcheck disable=SC1008,SC2034

supervisor=supervise-daemon
name="{ name }"
description="{ description }"
command="{ name }"
command_args="service --port={ port } --broadcast-port={ broadcast_port } --shutdown-command=\"{ shutdown_command }\" --hostname={ hostname }"
command_user="root"
pidfile="/run/${RC_SVCNAME}.pid"

export SHUTHOST_SHARED_SECRET="{ secret }"

depend() {
    need net
}
