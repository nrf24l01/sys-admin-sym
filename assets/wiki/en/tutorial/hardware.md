# Buy, assemble and power equipment

Start with the physical foundation. An Ethernet switch connects local devices;
a router connects IP networks. A server needs CPU, RAM, storage and usable
Ethernet interfaces before it can provide hosting.

## Shopping list

- One switch with at least five available Ethernet ports.
- Two physical servers: a controller and a VM node, each with at least two real
  Ethernet interfaces. Full packs include CPU, RAM, NIC and SSD; chassis-only
  purchases require installing compatible parts through the inspector.
- One edge router with a WAN interface and a LAN interface supporting the example.
- A bulk copper cable box and RJ45 plug packs, or suitable reusable finished leads.

Select **SHOP** to buy equipment. Select an inventory device and an empty rack
unit to place it. Compatible parts are installed from the server inspector. Use
the full-pack option for the shortest route through this lesson.

## Power and budget

Rack the switch and both servers, connect their power inlets to suitable rack
outlets or a powered PDU, then request power in the inspector. Rack power sockets
are distinct from Ethernet sockets. A device sitting in inventory cannot forward
packets. UPS/PDU equipment is useful for expansion, but rack outlets are sufficient
for this first setup when their capacity allows it.

Check prices before buying. If the starting balance is insufficient, **BANK**
shows the loan's payment schedule, interest and total repayment. Borrow only what
your purchases require. Payments use the hosting calendar. You can set calendar
speed to zero in **HOSTING → Market / subscriptions** while building; resume it
when waiting for customers. This also pauses loan time.

In Wiki, select which server is the controller and which is the VM node. The
controller will run the hosting service; it cannot also host customer VMs.

Next: [connect the management network](management-cables.md).
