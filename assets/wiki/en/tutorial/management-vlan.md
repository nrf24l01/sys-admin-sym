# Create the management VLAN

A **VLAN** splits a switch into separate Layer 2 broadcast domains. Devices in
VLAN 10 do not exchange ordinary Ethernet frames with devices in VLAN 20. This
keeps your private controller traffic separate from customer traffic.

An **access port** carries one VLAN and normally sends untagged frames to its
server. A **trunk** carries several VLANs with tags; we will use a trunk for the
router later. The private server interfaces here remain untagged.

## On the switch console

Confirm that ports 1 and 2 are the management cables, then enter:

```text
enable
configure terminal
vlan 10
name Management
exit
interface Gi1/0/1
switchport mode access
switchport access vlan 10
no shutdown
exit
interface Gi1/0/2
switchport mode access
switchport access vlan 10
no shutdown
exit
end
show vlan brief
show interfaces status
write memory
```

`vlan 10` creates the VLAN. `switchport access vlan 10` assigns a port to it.
`no shutdown` enables the interface. `write memory` saves the switch configuration
for device reloads. GUI port/VLAN settings satisfy the same checklist requirements.

Both management switch ports must share a declared VLAN other than default VLAN 1.
The guide accepts your own VLAN number; 10 is only this lesson's example. A VLAN
number is not an IP address or subnet mask.

Next: [assign private addresses](management-addresses.md).
More: [Layer 2 and VLANs](../reference/layer2-vlans.md).
