+++
title = "Architecture"
description = "The overall decisions and misc technical components."
weight = 1
+++

[[Slowshell]] is written in [Rust](https://rust-lang.org/) (common these days).
It's supposed to be lightweight, and have [[Plugins]] for custom application.

Slowshell is essentially a compositor for a global registry, one process that
owns and manages multiple layershell windows and route events to whomever needs
them.

The whole flow is:
  - [Daemon starts] -> [Collects Plugins] -> [Register Everything]
    -> [Starts event loop] -> [Initiate desktop items] -> [Sleep]

And unless an event is fired, nothing else is done.

## Desktop Items

The anchor of basically the whole [[Philosophy]]. DesktopItems are the actual
components that:
- listen to events
- make updates
- do actions
- render something

For example:
- panels are desktop items
- the spotlight is a desktop item
- and so on...

## Renderables

Renderables are free-roaming ui elements that can be rendered on surfaces
that allow renderables. They are like little plugs that can be socketed
on some parent (not like [[Components Overview|Components]]).

The reason they exist is to have miniature versions of desktop items, but
a lot more composable and drawable on any surface.
