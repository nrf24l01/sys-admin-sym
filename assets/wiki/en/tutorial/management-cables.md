# Connect the management network

A network is a set of devices able to exchange data through working connections.
An Ethernet cable connects two ports. A switch joins several ports at **Layer 2**,
forwarding frames within a VLAN using MAC addresses.

## Connect these ports

On each server, inspect the real interfaces with:

```sh
ip -br link
```

For the example, use `eth1` as management and reserve `eth0` for the public/data
network. Connect controller `eth1` to switch `Gi1/0/1`, and VM node `eth1` to switch
`Gi1/0/2`. Check your device's actual port names before following the examples.

Click a server's Ethernet socket, then the intended switch socket. Buy cable
materials first. Rack front/rear views can be changed while connecting. Passive
patch panels are allowed; bare room rack LAN sockets do not create an implicit
network between racks.

Both management ports should reach the same powered switch with operational
links. A cable may be present while its link is down because a device is off,
a port is shut down, or optical components are incompatible. Link LEDs and the
inspector show physical status; IP configuration cannot repair a down link.

Next: [place both ports in a management VLAN](management-vlan.md).
More: [Ethernet and networks](../reference/network.md).
