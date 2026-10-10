+++
title = "Popups"
description = "Popups for menus."
+++

In [[Slowshell]], popups are where overlay content is displayed, such as menus and alike.

> **Also**:  The popup is just a generic module, it has no direct if you're intending to
> just normally use slowshell.

## IPC

If you'd like, you can externally open(or close) existing menus as such:

```sh
slowshell ipc popup.open content=menus/network x=panel,Main y=cursor,10.0
slowshell ipc popup.toggle content=menus/quick
slowshell ipc popup.close
```

| Command | Args | Effect |
|---|---|---|
| `popup.open` | `content=` (required), `x=`, `y=`, `edge=`, `insets` | Opens a popup. |
| `popup.toggle` | same as `popup.open` | Toggle... |
| `popup.close` | none | Close whatever is open. |


- **Where:** `x` and `y` are each a `kind,value` pair:
  | Value | Meaning |
  |---|---|
  | `fixed,<px>` | The position default being (`fixed,0.0`). |
  | `cursor,<px>` | Position from pointer. |
  | `panel,<name>` | Anchor to a panel, e.g. `x=panel,Main`. |

  `edge=` snaps the popup to a panel edge: `left` or `right`.
  
- **And Where**: `content` is a [[Architecture#Renderables|renderable widget]] is displayed within the popup (same as [[Panels#Groups|`menu "<renderable>"`]]).
  You can view all possible values with:
  ```sh
  slowshell list renderables
  ```
  
  Built-ins: `menus/quick`, `menus/network`, `menus/sound`, `menus/power`,
  `menus/bluetooth`, `menus/notifications`, `menus/tray`, `menus/clock`, `menus/actions`,
  `menus/system`.

---
