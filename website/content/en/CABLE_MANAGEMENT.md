# Cable management

The inventory sidebar contains saved **RJ45 cable setup** defaults. Automatic
length follows the endpoints and selected rack or ceiling anchors. Add an extra
percentage, a fixed length in meters, or both. The **Service loop** preset adds
10% plus 0.50 m; **Minimum** clears both. These are game presets, not installation
standards.

The routed minimum already includes the existing 5% installation allowance on
the two port legs. Spans between fixing points use their measured distance. The
extra percentage applies to that whole minimum, rounds up to a centimeter, and
the fixed extra length is then added. A 2.00 m minimum with 10% and 0.50 m extra
produces a 2.70 m cut. Copper leads cannot exceed the simulation's 100 m limit.

Manual cuts use exactly the entered length and ignore the automatic defaults.
Hover a destination port to preview the cut length, two RJ45 plugs, or a free
reusable lead. No materials are spent until the connection succeeds. While
connecting, **Undo anchor** removes the last fixing point; **Cancel cable** or
Escape cancels the operation.

**Prefer reusable leads** selects the shortest available lead of the chosen
jacket color that fits the route and the requested reserve. Its entire length
is retained. Without this option, and for all manual cuts, reuse requires an
exact length and color match. Unplugging returns a finished lead to inventory;
it never returns bulk cable or loose plugs.

Select a cable to see its route minimum and remaining length reserve. A route
that exceeds the cut length shows the shortage and the authoritative physical
link evaluator reports the fault. Reorder, remove, or clear its anchors to fit
the existing lead, or unplug it and connect a longer one. Rerouting, changing
defaults, and loading a save preserve new leads' physical cut lengths.

Fiber, DAC and AOC assemblies retain their catalog lengths and separate
inventory. RJ45 sizing defaults never resize these assemblies or spend copper
materials. Their route must fit their finished length.

`NetworkSim.cable_settings` persists `extra_percent`, `extra_cm`, and
`reuse_longer_leads`; `Command::SetCableSettings` validates updates. Old saves
without settings keep the previous minimum-length and exact-reuse behavior.
The existing legacy automatic-length migration remains for older leads.

UI previews:

- [Before (English)](screenshots/cable-management-before-en.png)
- [After (English)](screenshots/cable-management-after-en.png)
- [After (Russian)](screenshots/cable-management-after-ru.png)
