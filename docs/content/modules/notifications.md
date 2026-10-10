+++
title = "Notifications"
description = "Desktop notifications."
+++

[[Slowshell]] has a built-in __optional__ notification daemon with basic 
functionality.


> **Note**: The [[Components Overview#core/notifications|`core/notifications`]] works alongside the daemon,
> you can check the dedicated page for more information.

## Configuration

```kdl
notifications {
  enabled #true // no hot reload
}
```

> **Note**: If the `notifications { }` block is present, `enabled` is **required**, otherwise the
> config fails to parse without it while when the block is absent entirely, the default applies
> and everything just runs.

> **Note**: The `notifications` config actually gates [[Components Overview#core/notifications|`core/notifications`]].

## Style classes

```css
/* The notification "card" itself */
.notifications {
  background-color: var(--crust-a90);
  border-radius: 10px;
  border-width: 1px;
  border-color: var(--overlay-a15);
  padding-v: 12px;
  padding-h: 12px;
  gap: 4px;
}

/* The history (the menu) list */
.notifications.list { gap: 8px; }

/* The rest */


.notifications.icon { icon-size: 20px; }
.notifications.app { color: var(--overlay); }
.notifications.app.icon-color { icon-size: 18px; }


.notifications.text-app { color: var(--overlay); font-size: 10px; }
.notifications.text-summary { font-size: 13px; }
.notifications.text-body { font-size: 11px; }

.notifications.input { font-size: 11px; padding: 6px; }

.notifications.action {
  background-color: var(--mantle);
  color: var(--text);
  border-radius: 8px;
  border-width: 0px;
  border-color: var(--overlay-a15);
  font-size: 11px;
  font-weight: 400;
  padding-v: 7px;
  padding-h: 10px;
}
.notifications.action:hover { background-color: var(--overlay-a15); }
.notifications.action:active { background-color: var(--overlay-a25); }
```

---
