#!/sbin/openrc-run
# shellcheck disable=SC1008,SC2034

supervisor=supervise-daemon
name="{ name }"
description="{ description }"
command="{ name }"
command_args="service --port={ port } --broadcast-port={ broadcast_port } --shutdown-command=\"{ shutdown_command }\" --hostname={ hostname } --init-system openrc"
command_user="root"
pidfile="/run/${RC_SVCNAME}.pid"

export SHUTHOST_SHARED_SECRET="{ secret }"

depend() {
    need net
}

stop() {
    # Send SIGTERM first for graceful shutdown
    start-stop-daemon --stop --pidfile "${pidfile}" --signal TERM --retry 5
    # If still running after 5 seconds, force-kill with SIGKILL
    if start-stop-daemon --test --stop --pidfile "${pidfile}"; then
        start-stop-daemon --stop --pidfile "${pidfile}" --signal KILL
    fi
}
