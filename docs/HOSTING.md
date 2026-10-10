# Hosting gameplay

Buy server hardware and address ranges in the existing shop and IP RANGES window.
Both `/29` and `/24` inventories are available. A `/24` contains 256 addresses;
network, broadcast, gateway and actual infrastructure assignments cannot be sold
to guests. Purchases use the existing money balance and owned address inventory.
A chassis needs CPU, RAM and an installed disk of the product's storage class.
The full server pack includes these parts.

Buy a **separate physical server** for the provider site and node controller.
Connect its public interface to the customer network and assign a usable public
IPv4 from an owned range. Connect another interface (usually **mgmt0**) to a
private management network and assign its internal IPv4. Configure transit and
return routes for the public address through the existing network tools.

In this server's existing console run:

```sh
./setup-main-hoster-server.sh
```

The script installs, enables and starts `hosting-provider.service` **on the machine
where it runs**, listening on that machine's configured network interfaces. No IP
arguments are required. It does not create addresses or routes. Running it again
is safe. The main server is reserved for this role and cannot host customer VMs.

In **HOSTING → Provider site**, enter **only its public IPv4** and click **Connect
hoster**. This works like a site's destination address: the simulation resolves
the actual physical machine that owns the address. There is no server selector
or internal IP field. The destination must have the app installed and a working
public transit/return route before customers can buy. Storefront traffic follows
that route, contributes to actual port counters and competes for shared uplink
capacity; congestion reduces successful visits and purchases. Disconnecting this
address does not uninstall or stop the server's service. Inspect, stop or restart
`hosting-provider` with the existing console's `systemctl` commands.

Connect each virtualization node's **mgmt0** to that private network and configure
its IP. In that node's server console run, using the provider server's **internal**
address (not its public address or the router's address):

```sh
./vm-prepare.sh 10.0.0.10
vm-host status
```

Preparation requires a powered operational node and a successful packet path
from mgmt0 to the installed main hoster server. Internal registration can precede
connecting its public address; placement requires both connections. Registration
is saved and idempotent.
A public storefront outage pauses new sales. Internal controller outages prevent
preparation, placement and interactive start/stop/reboot/destruction/recreation; running
guests continue on the data network. Existing subscriptions continue their normal
prepaid renewal/cancellation lifecycle. Replacing the controller server or changing
its internal IP requires nodes to run preparation again; existing VMs are retained.
The app accepts configured private addresses automatically. Switching the public
site address to another address on the same server retains node registrations.

Connect each node's **data** interface to the customer network. In **HOSTING →
Hosting nodes**, select the node and configure **Node IPv4 allocation ranges**.
Choose one or more owned ranges, or enter a smaller CIDR inside an owned range.
For example, buying `203.0.113.0/24` allows node 01 to use `203.0.113.0/29`
and node 02 to use `203.0.113.8/29`. Entering `203.0.113.1/29` selects the
canonical subnet `203.0.113.0/29`. Click **Apply node IPv4 ranges**. By default,
**Use product ranges automatically** inherits the published products' selections;
disabling it with no selected ranges blocks future IPv4 orders on that node.

Prepared nodes automatically obtain data-interface addresses from their permitted
ranges, with the subnet mask and gateway of a router actually reachable through
the connected cable and VLAN. A `/29` allocation slice may use a router's larger
`/24` interface subnet; the allocation slice still restricts guest addresses.
The allocator preserves manual interfaces and checks infrastructure, leases and
guest assignments to avoid duplicates. Configure the router, VLANs, transit and
return routes with the existing network tools. Ownership alone does not create
routes, and an isolated node cannot acquire a working gateway automatically.
After reconnection, addressing is retried at the next hosting interval.

In the management UI's node panel, **VM subnet → port / VLAN rules** defines
where a guest network interface connects when its IPv4 comes from a subnet.
Disable **Use existing node networks automatically**, select an owned subnet,
choose a physical data port and **Tagged VLAN** (1–4094) or **Native / untagged**,
then add and apply the rules. For example:

- A VM receiving `203.0.113.2` from `203.0.113.0/29` uses `eth0`, VLAN 10.
- A VM receiving `203.0.113.10` from `203.0.113.8/29` uses `eth1`, VLAN 20.

Multiple subnets can share a port, and different VLANs can share one trunk port.
Subnets on one node must not overlap. Management ports cannot carry guest traffic.
Guest addresses satisfy product ranges, optional node restrictions and these rules.

Rules configure the guest's bridge/VLAN and do not allocate or assign a public
IPv4 to the node's data interface. The node needs management connectivity; its
guest uplink can be an IP-free bridge. Provisioning discovers a real router
through ARP on the selected wire and uses its subnet mask and gateway for the VM.
Configure switch trunk/native VLANs, router interfaces and return routes using
the existing network controls. Disconnected or unmapped ports/VLANs cannot supply
new guests. Existing guests keep their IPs and networks after rule changes.

Guest rules and VLAN bridge transport persist in saves. Older nodes retain the
existing automatic-addressing mode. Previously assigned host aliases still needed
by live guests remain until those guests are removed; new explicit rules never
consume additional host IPv4 addresses.

Open **HOSTING**. Create a product, select its eligible servers and one or more
owned IPv4 ranges, configure resources and limits, then save it. Select the saved
product in the list and publish it. Publishing validates configuration but reserves
nothing. Live orders check management, hardware, storage class, overcommit policy,
data connectivity and address inventory before any mutation. The first usable
address in each owned pool is reserved for its gateway, alongside infrastructure
addresses, DHCP leases and all existing VM addresses. Addresses are shared by
products, but each order uses only the intersection of its product's and host
node's permitted ranges. Changing node selections affects future provisioning;
existing guests keep their addresses and supporting interface configuration.
Automatically assigned node addresses are released when no longer permitted and
no guest still uses that interface. Node policies and automatic addresses persist
alongside the existing allocations in saves.

The node's existing console shows its guest interfaces and assigned addresses in
`ip a`, `ip -br a`, `ip -o addr` and `ip addr show dev vnet<ID>`. `ip link show`
uses actual virtual link status; `ip -s link show` includes counters. Stopped guests show DOWN while
retaining their reserved addresses. Termination removes their interfaces from
these lists. Host interface edits remain limited to physical interfaces so guest
address ownership cannot be changed accidentally through host diagnostics.

The simulation owns all customers. Market prospects arrive progressively and
compare category-specific resource, performance, reliability and price utility
against a competitive benchmark. The Products tab shows expected daily orders,
category scores and actual sales. Existing services retain their purchased
specifications, price and ranges when a product is edited, unpublished or deleted.

Attractiveness is a weighted geometric mean of resource fit and affordability,
multiplied by observed service quality, reliability and provider reputation:

```text
affordability = 2 × customer_monthly_budget / (customer_monthly_budget + monthly_price)
attractiveness = product(resource_fit[i]^weight[i]) × affordability^price_weight
                × quality^(1 + performance_expectation)
                × reliability^(3 × reliability_expectation) × reputation
```

Resource fits compare effective CPU, RAM, disk capacity, actual disk IOPS and
network capacity with the customer's requirements, capped at 2 for diminishing
returns on unused resources. All weights sum to 1; the price weight depends on
category and individual price sensitivity. Affordability equals 1 at the customer's
budget and rises smoothly towards 2 as price falls. Every discount improves the
score, including below half the budget, while very high prices approach zero
utility. Poor performance and outages still reduce the score. Expected demand and
automatic purchases use this same score; saving price edits refreshes demand.

## Physical resources

The Hosting nodes tab configures independent CPU, RAM, virtual-storage,
storage-performance and network overselling percentages. Policies constrain
future placements; reducing them does not delete existing services.

When an order cannot fit, product availability and provisioning errors identify
each affected node and every failing resource: dedicated/shared CPU, RAM, physical
storage, virtual disk capacity, disk IOPS, disk throughput and NIC bandwidth.
Each line shows the requested amount, remaining capacity, existing commitments,
total after the order, policy limit and shortfall. Physical capacity, hypervisor
reservations, storage class/provisioning mode and the relevant overcommit percentage
explain the limit. These are admission commitments; live workload utilization is
shown separately in node and VM metrics. The same checks apply to VM recreation.

* CPU capacity uses installed physical cores times clock MHz. A vCPU is a
  3400 MHz compute entitlement, independent of SMT threads. Dedicated guests
  receive reserved CPU first; shared guests compete by weight, with idle shares
  redistributed. Sustained limits and finite burst credit constrain actual work.
  Metrics include completed compute, CPU Ready/Steal and limit throttling.
* RAM uses installed module capacity, without the existing UI's generation-weighted
  memory score. Allocated RAM and working memory are separate. Under demand,
  ballooning reclaims cache, cold pages swap and consume disk I/O, and OOM kills
  the largest working set only after resident RAM and reserved swap are exhausted.
  A workload exceeding its own guest memory allocation encounters guest-process
  allocation failures, recorded separately; its VM and NIC keep running and host
  OOM counters do not increase. Only actual exhaustion of host RAM and swap can
  stop a VM.
* HDD and SSD are separate pools built from installed drive capacity, IOPS and
  throughput ratings. Thick disks reserve their full size; thin disks start with
  1 GB of configured guest data and grow through completed writes. Capacity,
  IOPS, throughput, bounded operation queues and write failures affect workloads.
  A full guest disk rejects writes separately from physical pool exhaustion.
* VM interfaces are ordinary saved IPv4 ports attached to a host bridge in the
  existing Ethernet engine. ARP, VLANs, packet policy, static/BGP routes, optical
  negotiation and return paths determine reachability. Aggregate traffic uses
  traced routes, physical NIC rates and transit commitments. Shared bounded
  queues produce delay and loss; counters record actual served bytes and frames.
  The traffic limit applies to each prepaid subscription period.
* Dedicated physical-server products place exclusively on one physical node.
  Their service still uses the same customer, subscription and network lifecycle.

Category workloads include daily patterns, seeded spikes, idle intervals, build
bursts, sustained compute, disk consumption and transfers. Contention reduces
completed work and satisfaction; there are no random noisy-neighbor penalties.
Service quality measures delivery of work within the purchased CPU, RAM, disk I/O
and network limits. Requests above those limits, a full purchased guest disk, and
an exhausted prepaid traffic allowance do not count against provider reputation,
availability or retention. Customer-controlled guest shutdown, reboot or guest OOM
also does not count as a provider outage. While a guest is stopped, the simulation
checks whether its physical node, management connection and actual network/return
route would support a customer restart, using the existing packet engine without
generating guest utilization or traffic. Simulated customers automatically retry
their starts at hosting intervals when node management is available. Recovery does
not recreate services, addresses or payments, and recovery deadlines persist.
Actual physical contention, swapping, hypervisor host OOM, storage exhaustion,
node failures and route failures still affect quality. Hypervisor OOM is recorded
separately from guest stops and retries after a cooldown; recovery does not mask
continued physical RAM exhaustion. Red warnings describe these provider failures;
full guest disks, guest RAM limits and customer-controlled guest power states use
normal text. Older unclassified stopped/OOM guests can be recovered by their
simulated customers after loading, without a player start action.

## Time, subscriptions and persistence

The Market / subscriptions tab controls addressable market population, daily prospects, hosting clock speed and
positive weighted subscription periods. By default one real second advances
hosting by one minute. Workloads and market arrivals aggregate in five simulated
minute intervals. Set speed to zero to pause hosting. A month is 30 simulated days.
The top bar's **Speed** selector offers **×1, ×2, ×3, ×5, ×10, ×50 and ×100**.
It multiplies elapsed time for the existing power, packet and hosting clocks;
hosting calendar speed is applied afterwards. With the default calendar, ×100
advances 100 hosting minutes per real second. Workload accounting, orders and
renewals follow the same simulated intervals at every speed. Changing speed keeps
partially elapsed intervals, and the selected multiplier persists in saves.

Purchases and renewals are prepaid, recorded in cents and credited to the existing
integer money balance with a saved cent remainder. Customers pay exactly at
expiration or cancel according to observed quality. No arrears or expenses are
introduced. Termination removes VM interfaces, storage/resource reservations and
addresses; payment/service history remains. Successful order IDs are idempotent,
including after cancellation or loading, while failed orders consume no resources,
IDs, RNG draws or payments.

**HOSTING → Assigned IPv4** lists occupied owned addresses, their ranges, physical
nodes, guest interfaces, customers and services. Filter by node and open a service
or node console directly. Retained service addresses appear without a guest NIC
and still count as allocated in range availability.

In **Customer services**, **Destroy VM (keep IPs)** deletes the guest, its disk and
virtual interface, releasing its CPU, RAM and physical storage. Its paid contract,
expiration, customer and IP lease remain. **Recreate VM** installs a fresh guest on
the original physical node with the purchased specifications and exactly the same
addresses, without creating another payment. Recreation checks current management,
resource policies and data-interface compatibility before changing anything. An
unpublished/deleted product or public storefront outage does not invalidate a paid
contract; the internal management connection must still work. Destroying a guest
is distinct from customer shutdown/reboot: a paid service missing its guest counts
as a provider outage until restored. It is not automatically recreated or renewed;
at prepaid expiration it terminates and releases its addresses.

**Refund $… & release IPs** permanently ends the service, removes any remaining
VM and returns the unused part of its current prepaid period, rounded down to a
cent. Past periods remain paid. This settlement also works with an offline node.
Refunds debit the existing money balance with the same cent remainder as payments;
there is no new invoicing or expense system. Payment and refund history stays in
**Market / subscriptions**. Repeated recreation, cancellation and refund requests
remain safe across saves; reserved addresses also block the existing node and
DHCP allocators. Existing saves capture the purchased VM contract on destruction.

The existing SQLite/RON save contains products, node preparation and policies,
VMs and purchased specifications, customer state, services, retained IPv4 leases, payment/refund history,
clock, RNG, reservations and virtual interfaces. Older saves default to empty
hosting state and receive both setup and preparation scripts. Existing hosting saves without
a provider server keep their VMs and subscriptions; install the main hoster app,
connect its public address and re-register nodes to resume new provisioning. Older
server/public/private selections migrate to a public destination address while
retaining the installed service and node registrations. Only the original distributed
preparation script is upgraded to accept arguments; player-edited scripts are kept. Learned ARP/MAC state and
network queues are rebuilt after load; service identity and allocations remain.

The current hardware catalog offers SATA HDD and SSD; no NVMe is advertised.
The existing packet engine supports IPv4 and ICMP, not IPv6 or application TCP/UDP
sessions. Customer application traffic is aggregated over those real routing
paths, rather than simulating individual HTTP requests or game protocol packets.
Products without a public address have no Internet endpoint in this IPv4 model.
