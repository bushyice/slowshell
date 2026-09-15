spotlight:
  echo -n "exec spotlight.toggle" | socat - UNIX-CONNECT:/tmp/slowshell.sock

notif:
  echo -n "exec notification.new app=aa summary=ddhd body=dudb" | socat - UNIX-CONNECT:/tmp/slowshell.sock

bars:
  echo 'exec panel.create name=Secondary position=left height=48' \
    | socat - UNIX-CONNECT:/tmp/slowshell.sock

  echo 'exec panel.secondary.add_item section=center label=CPU component=core/cpu' \
    | socat - UNIX-CONNECT:/tmp/slowshell.sock

popup:
  echo -n "exec popup.open content=wifi x=panel,Main y=cursor,10.0" | socat - UNIX-CONNECT:/tmp/slowshell.sock

toggle:
  echo 'exec panel.main.toggle' | socat - UNIX-CONNECT:/tmp/slowshell.sock

run:
  pkill swaybg
  cargo run daemon

test:
  cargo nextest r
