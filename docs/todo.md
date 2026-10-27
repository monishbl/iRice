# iRice: later

## Squircle corners everywhere

Glass surfaces (dock, panels, Control Center, menus, island, cards, widgets)
use continuous-curvature (squircle) corners. Bring the same shape to every
other rounded shape too: buttons, text fields, list rows, Settings groups,
chips, badges. That needs one shared continuous-corner shape (a small
shader or a Shape path) that replaces plain `Rectangle { radius }` across
the shell.
