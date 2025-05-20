#!/bin/sh
# /etc/rc.d/rc.{name}
# {description}

case "$1" in
  start)
    echo "Starting {name}..."
    {binary}  control-service --config {config_location} &
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
