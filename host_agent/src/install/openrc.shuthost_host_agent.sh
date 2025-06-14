#!/sbin/openrc-run

supervisor=supervise-daemon
name="{name}"
description="{description}"
command="{binary}"
command_args="service --port={port} --shutdown-command=\"{shutdown_command}\""
command_user="root"
pidfile="/run/${RC_SVCNAME}.pid"

export SHUTHOST_SHARED_SECRET="{secret}"

depend() {
    need net
}
