# Switch and router console guide

This project implements an IOS-style command interpreter connected to its semantic
network simulator. It does **not** execute Cisco firmware or implement the whole
IOS/IOS XE operating system. Unsupported commands return an error; they do not
silently succeed.

## Cisco references

The implementation follows the public command syntax and console workflows in:

- [Catalyst 1000 IOS 15.2(7)Ex: Using the Command-Line Interface](https://www.cisco.com/c/en/us/td/docs/switches/lan/catalyst1000/software/releases/15_2_7_e/command_reference/b_1527e_1000_cr/using_the_command_line_interface.html)
- [Catalyst 1000: VLAN trunk configuration](https://www.cisco.com/c/en/us/td/docs/switches/lan/catalyst1000/software/releases/15_2_7_e/configuration_guides/vlan/b_1527e_vlan_c1000_cg/configuring_vlan_trunks.html)
- [ISR 1000: Using Cisco IOS XE Software](https://www.cisco.com/c/en/us/td/docs/routers/access/1100/software/configuration/xe-17/isr1100-sw-config-xe-17/cisco_1100_series_swcfg_chapter_0100.html)
- [ISR 1000 software configuration guide](https://www.cisco.com/c/en/us/td/docs/routers/access/isr1100/software/configuration/guide/isr1100-sw-config.html)

These are external manufacturer manuals, not bundled firmware or a promise that
all documented Cisco commands are supported here. The examples below describe
this simulator's implemented behavior.

## Open a console

Select a switch, router, or one of its ports. Its console appears below the rack.
Power the device on in the Inspector. A console connection does not need an
Ethernet cable; Ethernet cables carry simulated traffic between devices.

Each device has independent input, command history, output, and command mode.
Enter submits a command. Up/Down recalls history. Tab completes a unique fixed
command or lists matching syntax. `?` and `show ?` list available commands.
Ctrl-Z returns from configuration mode to privileged EXEC. Use **Paste
configuration** for multiline scripts and click **Run**; execution stops at the
first error, preserving earlier successful lines. Scripts are not a transaction,
but each individual command, including an interface range, is atomic.

## Command modes

| Prompt | Mode | How to enter |
| --- | --- | --- |
| `Switch>` / `Router>` | User EXEC | Initial console session |
| `Switch#` | Privileged EXEC | `enable` |
| `Switch(config)#` | Global configuration | `configure terminal` |
| `Switch(config-vlan)#` | VLAN configuration | `vlan 20` |
| `Switch(config-if)#` | Interface configuration | `interface Gi1/0/1` |
| `Switch(config-if-range)#` | Interface range | `interface range Gi1/0/1 - 4` |
| `Router(config-subif)#` | Router subinterface | `interface Gi0/1/0.20` |

`exit` leaves one configuration level. `end` returns to privileged EXEC.
`disable` returns to user EXEC. Unambiguous keyword abbreviations such as `en`,
`conf t`, `int`, `no shut`, and `sh ip int br` work. Ambiguous abbreviations fail.
Use `do show ...`, `do ping ...`, or `do traceroute ...` without leaving
configuration mode. `do write memory` also saves from configuration mode.

## Interface names

| Device | Console names | Rack labels |
| --- | --- | --- |
| Catalyst C1000 | `Gi1/0/1`–`Gi1/0/24` | `Gi1/0/01`–`Gi1/0/24` |
| Catalyst SFP cages | `Gi1/0/25`–`Gi1/0/28` | SFP ports, cabling unavailable |
| ISR C1111 WAN | `Gi0/0/0`, `Gi0/0/1` | WAN1, WAN2 |
| ISR C1111 LAN | `Gi0/1/0`–`Gi0/1/7` | LAN1–LAN8 |

Full `GigabitEthernet` names and existing rack labels also work. Router
subinterfaces accept `.number`; use `encapsulation dot1q VLAN` before assigning
an IP address. A subinterface number and its VLAN do not have to be equal.

The C1111 LAN ports currently use the simulator's **routed-port model**, which
differs from the real device's embedded Ethernet switch. Router LAN switchport
commands, bridge domains, and switch virtual interfaces are not implemented.

## Example: two server VLANs through a router

Install and power one switch, one router, and two servers. Cable server A to
switch port 1, server B to switch port 2, and router LAN1 to switch port 3.

Run on the switch, beginning at `Switch>`:

```text
enable
configure terminal
hostname Core
vlan 20
name Servers
exit
vlan 30
name Services
exit
interface Gi1/0/1
description Server A
switchport mode access
switchport access vlan 20
no shutdown
exit
interface Gi1/0/2
description Server B
switchport mode access
switchport access vlan 30
no shutdown
exit
interface Gi1/0/3
description Router LAN1
switchport mode trunk
switchport trunk allowed vlan 20,30
no shutdown
exit
end
write memory
```

Run on the router, beginning at `Router>`:

```text
enable
configure terminal
hostname Edge
interface Gi0/1/0.20
encapsulation dot1q 20
ip address 10.0.20.1 255.255.255.0
exit
interface Gi0/1/0.30
encapsulation dot1q 30
ip address 10.0.30.1 255.255.255.0
exit
interface Gi0/1/0
no shutdown
exit
end
copy running-config startup-config
```

Configure server A in the Inspector as `10.0.20.2/24`, gateway `10.0.20.1`, VLAN
20. Configure server B as `10.0.30.2/24`, gateway `10.0.30.1`, VLAN 30.
From server A's terminal, `ping 10.0.30.2` checks routing and `ping 10.0.20.1`
checks the gateway. The router console can `ping 10.0.20.2` or
`traceroute 10.0.30.2`.

WAN1 cabling alone does not provide Internet reachability. Configure a transit
range delivery in **IP RANGES** and explicit forward/return routes in
each router's **Routing table…** editor. Private-address examples above do not receive implicit NAT.
See [provider network operations](PROVIDER_NETWORK_GUIDE.md). No host Internet
packets are sent.

## Static routes from the router console

In global configuration mode, add a default route through the higher network
gateway shown in **IP RANGES**:

```text
enable
configure terminal
ip route 0.0.0.0 0.0.0.0 192.0.2.5
end
show ip route
ping 192.0.2.5
ping 8.8.8.8
write memory
```

The next hop selects the most specific configured, directly connected interface
in routing domain zero. If multiple interfaces match equally, specify the
interface: `ip route 0.0.0.0 0.0.0.0 Gi0/0/0 192.0.2.5`.
Use `no ip route NETWORK MASK NEXT-HOP` (or the interface form) to remove it.
Routes appear in the GUI, `show ip route`, and `show running-config`; `write
memory` saves them for reload. Interface configuration can switch directly to
another interface using `interface NAME`.

## Supported configuration

- Global: `hostname NAME`, `no hostname`, `vlan ID`, `no vlan ID`, `interface
  NAME`, `interface range Gi1/0/1 - 4`. VLAN mode: `name NAME`.
- Interfaces: `description TEXT`, `no description`, `shutdown`, `no shutdown`.
  Shutdown applies to physical ports, including all their subinterfaces.
- Switch ports: `switchport mode access`, `switchport access vlan ID`,
  `no switchport access vlan`, `switchport mode trunk`,
  `switchport trunk native vlan ID`, `switchport trunk allowed vlan LIST`,
  and allowed-list `add LIST` / `remove LIST`.
- Router global routes: `ip route NETWORK MASK NEXT-HOP`,
  `ip route NETWORK MASK INTERFACE NEXT-HOP`, and their `no` forms.
- Router interfaces: `ip address ADDRESS MASK`, `no ip address`;
  router subinterfaces additionally support `encapsulation dot1q ID`.
- Physical interfaces support `speed 10`, `speed 100`, `speed 1000`, and
  `speed auto` (the default). The configured advertisement is saved by
  `write memory`; a link negotiates the lower rate advertised by its endpoints.

VLAN IDs are 1–4094. Lists accept `20,30-35`, `all`, or `none`. Configure trunk
mode before changing its allowed/native VLAN settings. Trunks initially allow
all VLANs, but a VLAN must exist in the switch database before forwarding.
An access assignment creates a missing VLAN. VLAN 1 cannot be deleted.

The current port model stores only the active switchport mode. Switching from
trunk to access resets the access VLAN to 1; switching back to trunk restores
the default allowed list, not a previously configured inactive trunk profile.
Access/trunk forwarding applies VLAN tags and native-VLAN translation in the
Ethernet engine. An untagged server address can leave its VLAN affinity unset.

## Verify configuration and links

```text
show version
show running-config
show startup-config
show vlan brief
show interfaces
show interfaces status
show interfaces Gi1/0/1
show interfaces Gi1/0/1 switchport
show interfaces trunk
show ip interface brief
show ip route
show ip arp
show cdp neighbors
```

Outputs are computed from the simulator state. Interface status reflects power,
installation, administrative shutdown, cable length, and the remote cable
endpoint; the speed column reports the negotiated rate when the link is up and
the local advertisement when it is down.
CDP output derives active adjacent network devices from the topology; the
simulator does not exchange or age CDP packets. Router diagnostics support
connected destinations and explicitly configured transit paths. Traceroute lists
semantic hops, not measured packet timing. Switches cannot originate ping
because switch-originated diagnostics are not implemented; switch management
addresses can receive ping and SSH through an actual network path.

## Saving and reloading

`write memory` and `copy running-config startup-config` save the selected
device's configuration inside the simulation. `reload` asks for confirmation:
press Enter to restore startup configuration, or type `cancel` to keep the
running state. Unsaved changes are discarded only on confirmation. Reload
requires a saved startup configuration. Rack placement, power, cables, money,
and other devices are preserved. Per-device routing-domain bindings, BGP
sessions, filtering, DHCP pools, switch management and spanning-tree settings
are also restored. Carrier delivery contracts and address inventory remain
independent. DHCP leases and learned ARP entries are operational state, not
startup configuration. Older startup records without the new network snapshot
retain their previous reload behavior.

Use the game's **Save** button after commands finish to persist the simulation,
including both running and startup configurations, to SQLite. **Load** restores
those configurations and starts fresh console sessions. Power toggles preserve
running configuration in this simulator; use `reload` for startup restoration.

## Compatibility boundary

This is a functional IOS command subset. Firmware boot, authentication/AAA,
EtherChannel, QoS, IPv6, configurable IOS NAT, OSPF/EIGRP and SNMP are not modeled.
The Ethernet/IPv4 packet engine supports static-route forwarding. Provider-side
BGP policy, converged spanning-tree forwarding, interface filtering, switch
management and semantic DHCP have scenario configuration APIs; their complete
IOS command grammar and protocol-message exchange are not implemented.
Unsupported IOS commands fail explicitly. Use the Inspector for device
interfaces, the router's routing editor for static routes, and **IP RANGES**
for allocation delivery and WAN/LAN addressing instructions. `show ip route` includes static routes configured in the GUI.

Regression tests cover console modes, invalid/ambiguous commands, device-session
isolation, atomic range failure, VLAN switching, routed VLANs, gateway ping,
trunk filtering, shutdown, startup reload, and SQLite persistence.
