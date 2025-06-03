#!/sbin/openrc-run

supervisor=supervise-daemon
name="{name}"
description="{description}"
command="{binary}"
command_args="service --port={port} --shutdown-command=\"{shutdown_command}\" --shared-secret=\"{secret}\""
command_user="root"
pidfile="/run/${RC_SVCNAME}.pid"

depend() {
    need net
}