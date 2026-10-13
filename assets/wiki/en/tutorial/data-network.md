# Connect and route the customer network

Keep management on VLAN 10. Use VLAN 20 for the controller's public interface
and the VM node's guest bridge. Connect controller `eth0` to switch `Gi1/0/3`,
node `eth0` to `Gi1/0/4`, and router LAN `Gi0/1/6` to `Gi1/0/5`.

## On the switch

```text
enable
configure terminal
vlan 20
name Customers
exit
interface Gi1/0/3
switchport mode access
switchport access vlan 20
no shutdown
exit
interface Gi1/0/4
switchport mode trunk
switchport trunk allowed vlan 20
no shutdown
exit
interface Gi1/0/5
switchport mode trunk
switchport trunk allowed vlan 20
no shutdown
exit
end
write memory
```

The controller sends untagged traffic into its access port. The VM node's guest
bridge and the router use trunks carrying tagged VLAN 20. A router subinterface receives those tagged frames.

## On the example C1111 router

Use your IP RANGES handoff and gateway values in place of these examples:

```text
enable
configure terminal
interface Gi0/0/0
ip address 192.0.2.2 255.255.255.252
no shutdown
exit
interface Gi0/1/6
no switchport
no shutdown
exit
interface Gi0/1/6.20
encapsulation dot1q 20
ip address 203.0.113.1 255.255.255.248
exit
ip route 0.0.0.0 0.0.0.0 192.0.2.1
end
show ip route
ping 192.0.2.1
write memory
```

`no switchport` converts this C1111 flex LAN port to routed mode. Use a routed LAN
port on other models. The `/29` connected route covers the customer network.
The default route sends other destinations to the upstream gateway. Applying
range delivery in IP RANGES provides the upstream route **back** to this router.

## Map VM addresses to the node's guest bridge

In **HOSTING → Hosting nodes**, select your node. Under **VM subnet → port / VLAN
rules**, disable **Use existing node networks automatically**. Add the owned
`203.0.113.0/29` subnet on node `eth0`, using **Tagged VLAN 20**, and apply.
The node cable is a trunk, so the guest bridge sends VLAN 20 tags. The router
subinterface receives the same tags. Management remains untagged on its separate
access cable. Matching tags and allowed VLANs are required at both ends.

This explicit rule lets the node bridge guest traffic without consuming a public
IP for the host. Management continues on `eth1`. The router gateway must be
reachable through the real port/VLAN path; a saved rule alone is insufficient.

Next: [make the storefront reachable](storefront.md).
