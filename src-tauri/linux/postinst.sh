#!/bin/sh
# The core keeps its secrets in /var/lib/sikemux, which only the group
# sikemux may enter, and runs setgid to it. Nobody joins the group, so the
# agents a person runs cannot read them.
set -e
getent group sikemux >/dev/null || groupadd --system sikemux
chgrp sikemux /usr/bin/sikemux-editor
chmod 2755 /usr/bin/sikemux-editor
install -d -o root -g sikemux -m 770 /var/lib/sikemux
