# Layer 2, switches and VLANs

**Layer 2** moves Ethernet frames on a local network. Each interface has a MAC
address. A switch learns where MAC addresses are reachable and forwards frames
to suitable ports. Broadcast frames, including ARP discovery, stay in their VLAN.

A VLAN is a separate broadcast domain. VLAN 10 and VLAN 20 can use the same
physical switch while remaining separate. To exchange IP traffic between them,
you need routing and permitted paths.

An access port carries one VLAN. A server usually sends untagged frames; the
switch assigns them to the access VLAN. A trunk carries multiple VLANs identified
by 802.1Q tags. Its allowed list controls which VLANs can cross; a native VLAN
handles permitted untagged traffic.

Both ends must agree. A tagged server on an untagged access path, a missing allowed
VLAN, or mismatched native VLANs can break traffic despite an operational cable.
Creating a VLAN alone does not assign ports or IP addresses.

Useful switch commands:

```text
show vlan brief
show interfaces status
show interfaces Gi1/0/1 switchport
show interfaces trunk
```

See the [management VLAN lesson](../tutorial/management-vlan.md).
