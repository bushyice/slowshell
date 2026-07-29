spotlight:
  echo -n "exec spotlight.toggle" | socat - UNIX-CONNECT:/tmp/slowshell.sock

notif:
  echo -n "exec notification.new app=aa summary=ddhd body=dudb" | socat - UNIX-CONNECT:/tmp/slowshell.sock
  
  
