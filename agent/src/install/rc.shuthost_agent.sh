#!/bin/sh
# /etc/rc.d/rc.shuthost_agent
# {description}

case "$1" in
  start)
    echo "Starting shuthost agent..."
    {binary} service --port {port} --shutdown-command "{shutdown_command}" --shared-secret "{secret}" &
    ;;
  stop)
    echo "Stopping shuthost agent..."
    pkill -f '{binary}'
    ;;
  restart)
    $0 stop
    $0 start
    ;;
  *)
    echo "Usage: $0 {start|stop|restart}"
    ;;
esac
