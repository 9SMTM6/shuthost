#!/sbin/openrc-run

supervisor=supervise-daemon
name="{ name }"
description="{ description }"
command="{ binary }"
command_args="control-service --config { config_location }"
command_user="{ user }"
pidfile="/run/${RC_SVCNAME}.pid"

depend() {
    need localmount
    after bootmisc
}
