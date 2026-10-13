# Using device consoles

Select a powered device to use its console, or detach the console window. Each
server has a simulated Linux environment; switches and routers use IOS-style
commands. A command applies to the console's current device.

Use Tab for command completion and `?` in IOS for contextual help. Linux `help`
lists implemented commands. Wiki code blocks can be copied and pasted into the
console; replace example interface names and addresses first. Copying commands
does not execute them.

IOS starts in user mode. `enable` enters privileged mode; `configure terminal`
enters global configuration; `interface NAME` selects an interface. `exit` goes
up one level, `end` returns to privileged mode, and `write memory` saves settings
for device reloads.

Linux `ip` changes active network configuration. `/etc/network/interfaces` and
`systemctl restart networking` provide persistent configuration for reboot.
The `nano` editor uses Ctrl+O, Enter to save and Ctrl+X to exit.

You may use GUI configuration instead. The Wiki checks the resulting world, not
whether you copied a prescribed command sequence. The terminal implements the
game's supported commands, not arbitrary operating-system software.
