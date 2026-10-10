+++
title = "Philosophy"
sort_by = "weight"
+++

The entire [[Slowshell]] philosophy is about not doing unnecessary work. Therefore;
it's architecture leans towards being **update-based** so that things only refresh
when they absolutely need to.

You see, I wanted slowshell to do little to no work when idle, and when it does work,
I wanted it to only do work on paths that require that work. Basically, I didn't want 
something to wake the whole shell up and update components it doesn't need to: for example,
you wouldn't want changing a workspace to update the battery status, so the only one you
want to do real work remains only the workspace indicator, or whatever is related.

To sum it up, work is dependent on **who needs to work for an event**.

## I see, what else?

I am also a lover for software that allow imaginations to run wild; Not having some API
that does something in the application, but applications that have little platforms within
while respecting that application's boundaries (for example, you probably wouldn't want
your email client to run doom).

So [[Plugins|plugins]] were guaranteed from the start, that's part of why I made slowshell,
because having plugins could, not only allow others to actually build their own shells, but
also have opinionated application outside the base shell for those who would like it a
clean slate.

## The decisions

A few of the decisions I made were these:
- using [iced](https://iced.rs/) for ui
- using [toml](https://toml.io/) before I decided to go for [KDL](https://kdl.dev/) instead
- using native plugins because sandboxing didn't sound right for a shell
- using [abi-stable](https://crates.io/crates/abi-stable) instead of using C-ABI plugins

That's it, for any other technical details, you can read about in [[Architecture|here]].

## The inspiration(s)

- [quickshell](https://quickshell.org/)
- [dank shell](https://danklinux.com/)
- [azalea-shell](https://github.com/enzohideo/azalea-desktop-shell).
