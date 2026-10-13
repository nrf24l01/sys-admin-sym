# Obtain public addresses and upstream delivery

Your private management addresses are not public customer endpoints. Buy a public
IPv4 range in **IP RANGES**. A `/29` provides eight addresses: the network and
broadcast addresses cannot be sold, and this example reserves one address for
the router gateway and one for the storefront.

Select the purchased range, choose a room uplink, and click **Apply range delivery**.
Read the displayed values: higher network gateway, router WAN address/prefix,
and range LAN gateway. Ownership gives you addresses; delivery supplies the
carrier's return route to your edge router. You must still configure that router.

For a first-uplink example these may be:

- Handoff subnet: `192.0.2.0/30`.
- Higher network gateway: `192.0.2.1`.
- Router WAN: `192.0.2.2/30`.
- Purchased range: `203.0.113.0/29`.
- Customer LAN gateway: `203.0.113.1/29`.

**Use the values shown in your game**, especially if you chose another uplink
or purchased a different range. Documentation addresses here are simulated.

Cable the selected room uplink to the router WAN port, usually `Gi0/0/0` on the
example C1111. Place, power and enable the router.

Next: [configure the customer network and routes](data-network.md).
More: [Layer 3 and routes](../reference/layer3-routes.md).
