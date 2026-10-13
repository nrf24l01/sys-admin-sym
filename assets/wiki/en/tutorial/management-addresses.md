# Assign private management addresses

An **IP address** identifies an interface at Layer 3. The prefix tells a server
which addresses are directly on its network. In `10.0.10.10/24`, `/24` means the
network is `10.0.10.0` with mask `255.255.255.0`.

Give the controller `10.0.10.10/24` and the VM node `10.0.10.11/24`, on their real
management interfaces. Addresses must be different, private, unused elsewhere,
and in the same subnet. These two devices communicate directly through VLAN 10;
they do not need a default gateway for this local conversation.

## On the controller console

```sh
ip addr add 10.0.10.10/24 dev eth1
ip link set dev eth1 up
ip -br addr
```

## On the VM node console

```sh
ip addr add 10.0.10.11/24 dev eth1
ip link set dev eth1 up
ip -br addr
```

Use your actual interface names. Keep these interfaces untagged; the switch
access ports select VLAN 10. The server's Access VLAN field is not an instruction
to create a tagged Linux subinterface. Duplicate or old addresses can be removed
with `ip addr del ADDRESS/PREFIX dev INTERFACE`.

## Save Linux network settings for reboot

Runtime `ip` changes can be lost during a Linux reboot. On each server, edit:

```sh
nano /etc/network/interfaces
```

For the controller, enter:

```text
auto lo eth0 eth1
iface lo inet loopback
iface eth0 inet manual
iface eth1 inet static
    address 10.0.10.10/24
```

On the VM node use `10.0.10.11/24` instead. Ctrl+O, Enter writes the file;
Ctrl+X exits. Apply with:

```sh
systemctl restart networking
ip -br addr
```

Later we will add the controller's public address to its `eth0` stanza. The node's
`eth0` can remain an IP-free guest bridge when explicit guest subnet rules are used.

Next: [test with ping](management-ping.md).
More: [IP networks and routes](../reference/layer3-routes.md).
