#!/sbin/openrc-run

supervisor=supervise-daemon
name="{ name }"
description="{ description }"
command="{ name }"
command_args="control-service --config { config_location }"
command_user="{ user }"
pidfile="/run/${RC_SVCNAME}.pid"

depend() {
    need localmount
    after bootmisc
}

stop() {
    # Send SIGTERM first for graceful shutdown
    start-stop-daemon --stop --pidfile "${pidfile}" --signal TERM --retry 5
    # If still running after 5 seconds, force-kill with SIGKILL
    if start-stop-daemon --test --stop --pidfile "${pidfile}"; then
        start-stop-daemon --stop --pidfile "${pidfile}" --signal KILL
    fi
}
