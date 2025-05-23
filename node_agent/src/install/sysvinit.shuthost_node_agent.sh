#!/bin/sh
# {description}

case "$1" in
  start)
    echo "Starting {name}..."
    {binary} service --port {port} --shutdown-command "{shutdown_command}" --shared-secret "{secret}" &
    ;;
  stop)
    echo "Stopping {name}..."
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
