# Overlays

An overlay is a child element marked `.overlay(def)`: it leaves normal
flow, lays out against the canvas, positions relative to its anchor (the
parent), paints above everything, and hit-tests first.

The modes:

- `Overlay::menu()` — click the anchor to toggle; closes on outside
  click, Escape, or choosing a clickable inside (selects use this).
- `Overlay::tooltip()` — shows after a hover delay; never hit-tested.
- `Overlay::modal()` — open while present in the tree (app-driven), with
  backdrop and a focus trap; outside click/Escape emit `on_close`.
- `Overlay::toasts()` — app-driven stack pinned top-right; nothing closes
  it from outside.
- `Overlay::context()` — app-driven like modal, no backdrop; pins at the
  pointer position the moment it opens (right-click menus).

Kit wrappers: `tooltip(target, text)`, `modal(title)`, `toast_stack(..)`,
`dropdown_menu(items)`, `context_menu(items)`, `popover(content)`, and
`combobox(value, open, options)` — an editable select whose typing
filters the listbox.
Nested overlays (a select inside a modal) work. Enter animations are
200 ms fade (+slide for centered overlays); reduced motion snaps them.
