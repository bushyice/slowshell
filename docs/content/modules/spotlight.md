+++
title = "Spotlight"
description = "A simple launcher, search, and/or command palette."
+++

[[Slowshell|Slowshell's]] spotlight is a simple launcher with modes. That's kind-of it.

## Configuration

```kdl
spotlight {
  cache #true
  cliphist #true
}
```

| Node | Type | Default | Hot-Reload | Notes |
|---|---|---|---|---|
| `cache` | bool | `#false` | Yes | Cache = faster open. |
| `cliphist` | bool | `#false` | Yes | Uses `cliphist`. |

### Modes

A spotlight mode is basically what the spotlight searches through (or what it generates).
For example, applications (obviously), or just a simple calculator.

### Built-in modes

| Mode | What it searches | How to access |
|---|---|---|
| `applications` | Desktop entries, in a list or a grid | default |
| `clipboard` | Clipboard history (via `cliphist`) | IPC `spotlight.toggle clipboard` |
| `commands` | Psuedo-command-palette. | do `/` |
| `calculator` | It doesn't. | `23+2` |

### What about other modes

You can list all spotlight modes with this:
```sh
slowshell list spotlights
```

### Display styles

Some modes support more than one layout. `list` is the default, `grid` tiles results, and
`image-list` is for results with images (e.g: clipboard previews).

## Style classes

```css
.spotlight { padding-v: 14px; padding-h: 14px; gap: 8px; }

.spotlight.search {
  background-color: var(--mantle);
  border-radius: 12px;
  border-width: 1px;
  border-color: var(--overlay-a30);
}

.spotlight.result { background-color: var(--mantle); border-radius: 12px; border-width: 0px; border-color: var(--overlay-a15); }
.spotlight.result.selected { background-color: var(--primary); }
.spotlight.row { padding-v: 10px; padding-h: 10px; }
.spotlight.tag { background-color: var(--crust-a60); border-radius: 4px; border-width: 0px; border-color: var(--overlay-a15); }
.spotlight.action-chip { background-color: var(--crust-a60); border-radius: 4px; border-width: 0px; border-color: var(--overlay-a15); }

.spotlight.single {
  padding-v: 18px;
  padding-h: 18px;
  background-color: var(--mantle);
  border-radius: 12px;
  border-width: 1px;
  border-color: var(--overlay-a30);
}

.spotlight.backdrop { background-color: var(--crust-a60); }
```

## IPC

```sh
slowshell ipc spotlight.toggle
slowshell ipc spotlight.toggle clipboard
slowshell ipc spotlight.open applications style=grid
slowshell ipc spotlight.close
```

---
