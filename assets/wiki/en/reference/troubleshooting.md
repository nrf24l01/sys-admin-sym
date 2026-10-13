# Diagnose a broken hosting setup

Follow the path in order; each check narrows down the cause.

1. **Power and installation:** are devices racked, supplied with power, and on?
2. **Physical link:** inspect the socket status; cable presence is not enough.
3. **Layer 2:** check both access VLANs or trunk tags and allowed VLANs.
4. **Addressing:** confirm unique addresses, masks and selected source interface.
5. **Routes:** check next-hop reachability, default route and the return route.
6. **Service:** confirm the controller service is active and the node is registered.
7. **Placement:** read product capacity, subnet mapping and free-address diagnostics.
8. **Demand:** ensure time and prospects advance and the paid product is attractive.

Linux diagnostics:

```sh
ip -br addr
ip route
ip route get 10.0.10.10
ping -c 3 10.0.10.10
traceroute 8.8.8.8
systemctl status hosting-provider
vm-host status
```

Run controller service commands on the controller and `vm-host` on the VM node.
On network devices use `show interfaces status`, `show vlan brief`,
`show ip interface brief`, `show ip route` and `show running-config` as applicable.

Use the Wiki's current task messages to choose a check. A previously completed
first-sale milestone is historical; it does not certify that a later modified
network still works.
