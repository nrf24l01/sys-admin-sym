# Sell your first VM

Build a small provider, publish a VM offer, and let a simulated customer buy it.
The checklist reads your current world. You can use the GUI or the device consoles
and complete tasks in any order. Select your **controller** and **VM node** in this
window so the checks refer to the servers you intend to use.

## What you are building

The controller runs your storefront and coordinates virtualization nodes. The VM
node runs customer guests. These are **two separate physical servers**. Both join
an isolated private management network and a customer data network. An edge
router connects the customer network to the room's upstream handoff.

```text
Room uplink -- router -- switch, customer VLAN 20
                         |-- controller eth0 (public storefront)
                         |-- VM node eth0 (guest bridge)

                    switch, management VLAN 10
                         |-- controller eth1 (10.0.10.10/24)
                         |-- VM node eth1 (10.0.10.11/24)
```

One switch can carry both VLANs. They remain separate networks despite sharing
hardware. Only the router connects the public customer network to the upstream.
The management network needs no Internet gateway for these two servers to talk.

## Follow the path

1. [Buy, assemble, rack and power equipment](hardware.md).
2. [Connect the management ports](management-cables.md).
3. [Create a management VLAN](management-vlan.md).
4. [Assign private IP addresses](management-addresses.md).
5. [Verify connectivity with ping](management-ping.md).
6. [Install the controller and prepare the VM node](hosting-control.md).
7. [Buy a public range and configure its delivery](public-range.md).
8. [Connect and route the customer network](data-network.md).
9. [Bring the public storefront online](storefront.md).
10. [Publish a VM product](product.md).
11. [Wait for your first paid order](first-sale.md).

The examples use a C1000-style switch, C1111 router and R360 full packs. Equivalent
models work. Always inspect interface names and the actual range/handoff values;
replace examples when yours differ. The guide remains available after completion.

## How the checklist works

A green task means its current requirement is satisfied. Repairing a network can
restore a task; unplugging it can make the task pending again. The historical
**First paid VM sold** milestone stays complete after cancellation or later
reconfiguration. Ping checks use current packet reachability; you do not have to
run a specific command sequence to earn a checkmark.
