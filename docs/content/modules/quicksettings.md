+++
title = "Quick settings"
description = "A general menu."
+++

Basically, just a general menu to be rendered on a [[Popup|popup]]. The
thing about it is that it allows extensibility and serves as the hub
for some controls.


## How to access

By default, there is no [[Components Overview|Component]] that actually
opens this menu, however you can open it via something like this:

**With a component**:
```kdl
component "core/icon" {
  label "Quick settings"
  menu "menus/quick"
  // ...
}
```

**Via IPC**:
```sh
slowshell ipc popup.open content=menus/quick x=cursor,0.0 y=cursor,0.0
```
