# Publish a VM product

In **HOSTING → Products**, create a VM product, select your prepared VM node
(or its group), and choose your owned public range. The controller is reserved
for its service and cannot host customer guests.

Start with a modest offer your installed hardware can supply. For example:

- Shared CPU, 1 vCPU and 1 GB RAM.
- A small virtual disk, such as 10 GB, and one public IPv4.
- Network limit such as 100 Mbps.
- A positive monthly price, for example $5 as an initial experiment.

Match storage IOPS/throughput limits to the installed drives. A modest memory/disk
size does not compensate for requesting more disk IOPS than the host can supply.
The controller and gateway consume infrastructure addresses, so only the remaining
usable addresses can be sold. Avoid overselling during the first lesson.

Save the product, select it, and publish it. Publishing validates the offer but
does not reserve resources or guarantee a purchase. Every order checks live node
management, capacity, guest bridge, address availability and public connectivity.

Read **expected daily orders** and the availability diagnostics. If provisioning
is blocked, fix the specific port, route, capacity or address shortfall. If it is
ready but demand is low, compare price and resource fit to customer expectations.
There is no single price that guarantees an immediate sale.

Next: [wait for the first paid VM sale](first-sale.md).
