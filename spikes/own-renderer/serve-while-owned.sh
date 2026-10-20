#!/bin/sh
# **A web host that lives only as long as the process that started it**
# (track flake-hunt, 2026-10-20).
#
#   sh serve-while-owned.sh COMMAND [ARGUMENT]...
#
# Runs COMMAND, and ends it when this script's standard input reaches its end.
# Playwright starts each of the suite's hosts (`webServer`) with a pipe for its
# standard input, holds the pipe's other end as long as it runs, and stops the
# host as it ends. A runner that is killed outright (a recipe's bound, the
# kernel's out-of-memory killer, a cancelled job) stops nothing: each host is
# the leader of a process group of its own, which nothing then kills, and it
# lives on holding its port, so that every later run on the port fails to
# start ("is already used", 52 hosts at a time after e14-refusal's bound of
# 1,200 s). The kernel closes the runner's end of the pipe when the runner is
# gone, however it went, and the host ends with it, in a few milliseconds and
# with no one polling for its owner.
#
# Exits with COMMAND's status, as soon as COMMAND ends.
if [ $# -eq 0 ]; then
  echo "serve-while-owned: no command to run" >&2
  exit 2
fi

# Our standard input, kept for the watcher: the asynchronous commands of a
# shell without job control read /dev/null, whatever this script's input is.
exec 3<&0
"$@" 0</dev/null 3<&- &
host=$!
(
  while read -r _ <&3; do :; done
  kill -KILL "$host" 2>/dev/null
) &
watcher=$!
wait "$host"
status=$?
kill "$watcher" 2>/dev/null
exit "$status"
