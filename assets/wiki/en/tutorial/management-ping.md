# Check management connectivity with ping

`ping` sends ICMP echo requests and waits for replies. Success proves that the
source, physical links, VLAN transport, IP addressing and return path work
together. A lit port alone does not prove this.

## On the VM node

```sh
ip route get 10.0.10.10
ping -c 3 -I eth1 10.0.10.10
```

## On the controller

```sh
ip route get 10.0.10.11
ping -c 3 -I eth1 10.0.10.11
```

Both directions should report reachable. `ip route get` should select the private
management interface, not the public data interface. The checklist performs the
same current packet-path checks; reading a command or typing a failed ping does
not complete the task.

## If ping fails

Check in order: both devices are racked and powered; their Ethernet links are up;
both switch ports belong to the same VLAN; both private addresses are unique and
share a subnet; the Linux route selects the intended interface. Use:

```sh
ip -br addr
ip link show
ip route
ip neigh show dev eth1
```

On the switch use `show interfaces status` and `show vlan brief`. Correct the
cause and retry; adding a public default route is unnecessary for same-subnet
management traffic.

Next: [install hosting services](hosting-control.md).
More: [troubleshooting](../reference/troubleshooting.md).
