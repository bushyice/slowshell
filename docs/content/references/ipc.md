+++
title = "IPC"
description = "Details for IPC."
weight = 2
+++

[[Slowshell]] creates a [UDS](https://en.wikipedia.org/wiki/Unix_domain_socket) at
`$XDG_RUNTIME_DIR/slowshell.sock` before falling back to `/tmp/slowshell.sock`.

## Using the slowshell command

`slowshell ipc` is a small helper to help you communicate with slowshell WITHOUT
doing something like `echo -n "exec spotlight.toggle" | socat - UNIX-CONNECT:/tmp/slowshell.sock`.

```sh
slowshell ipc spotlight.toggle
slowshell ipc panel.main.toggle
```

## Cool, what's the point?

It's for any external application to be able to communicate with slowshell. For example,
if you want to bind a key into a slowshell action within your compositor (in this example niri),
you could use `slowshell ipc`:

```kdl
// niri/config.kdl
Mod+A hotkey-overlay-title="Slowshell Spotlight" {
  spawn "slowshell" "ipc" "spotlight.toggle"
}
```

## Commands

IPC Commands are events that can be triggered via IPC, for example you
can create a panel via `panel.create`.

These commands are special events that modules define in order to expose
for IPC and/or [[Spotlight#Modes|the spotlight command palette]].

## What are the available commands?

Most IPC commands are provided via modules,
so here's a list of the built-in IPC commands introduces via modules:

- [[Panels#IPC|Panels]]
- [[Spotlight#IPC|Spotlight]]
- [[Notifications#IPC|Notifications]]
- [[Wallpaper#IPC|Wallpaper]]
- [[Desktop Widgets#IPC|Desktop Widgets]]


---
