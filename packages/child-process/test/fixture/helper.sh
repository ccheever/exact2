#!/bin/sh
# The stage drives' real child (LLP 1133.007 §10): `--process
# path:fixture-child=<this file>` runs it in the program's place. It starts
# a grandchild that holds its output, says so, echoes each line of its
# stdin, and exits 0 on `quit`, leaving the grandchild holding the pipe.
# `close` closes its stdin and waits, so the app's next write finds no
# reader.
if [ "$1" = "--version" ]; then
  echo "fixture-child 1.0"
  exit 0
fi
sleep 1000 &
echo "listening $$ grandchild $!"
while read -r line; do
  case "$line" in
    quit) exit 0 ;;
    close) exec 0<&-; echo "stdin closed"; sleep 1000 ;;
    *) echo "echo $line" ;;
  esac
done
exit 0
