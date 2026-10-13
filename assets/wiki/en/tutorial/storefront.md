# Bring your public storefront online

The controller needs a usable public address from your owned range and a working
outbound and return route. For the example range, use `203.0.113.2/29` with gateway
`203.0.113.1`. Keep `10.0.10.10/24` on the private management interface.

## On the controller

```sh
ip addr add 203.0.113.2/29 dev eth0
ip link set dev eth0 up
ip route replace default via 203.0.113.1 dev eth0
ping -c 3 203.0.113.1
ping -c 3 8.8.8.8
systemctl status hosting-provider
```

Use your range's actual values. `8.8.8.8` is a simulated diagnostic host, not
traffic sent from your computer. If the gateway works but external ping fails,
check the router default route, WAN handoff and IP RANGES return delivery.

Update `/etc/network/interfaces` on the controller so a reboot restores both
networks:

```text
auto lo eth0 eth1
iface lo inet loopback
iface eth0 inet static
    address 203.0.113.2/29
    gateway 203.0.113.1
iface eth1 inet static
    address 10.0.10.10/24
```

Apply with `systemctl restart networking`. Then open **HOSTING → Provider site**,
enter **only** `203.0.113.2`, and click **Connect hoster**. The game resolves the
physical server owning that address. There is no URL path or private IP in this
field. The installed service and actual public route must both work.

A storefront outage pauses new sales. Running guest traffic and controller
management are separate paths; keep the management network isolated and working.

Next: [publish your first VM offer](product.md).
