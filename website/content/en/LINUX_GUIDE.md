# Linux guest console

Each server has its own simulated Debian-style Linux environment, files, shell
history, services, and network state. Use `help` for the implemented commands.
This is a deterministic game OS, not a Linux VM: it does not install or run native
binaries. Commands and flags described here are implemented; arbitrary Linux
packages, full Bash programming, IPv6, and real Internet services are outside
this guest's scope.

## Configure addresses and routes

```sh
ip a
ip -br addr
ip link show
ip link set dev eth0 up
ip addr add 10.0.0.10/16 dev eth0
ip addr add 192.0.2.10/24 dev eth0
ip addr del 192.0.2.10/24 dev eth0
ip route
ip route add 198.51.100.0/24 via 10.0.0.1 dev eth0 metric 100
ip route replace default via 10.0.0.1 dev eth0
ip route get 198.51.100.5
ip route del default
ip neigh show dev eth0
ip neigh flush dev eth0
ping -c 3 -I eth0 10.0.0.20
traceroute 10.0.0.20
```

Addresses affect actual simulated Ethernet/ARP/ICMP traffic. Multiple addresses
per interface are supported. Routes use longest prefix, then lowest metric.
`ip addr flush dev eth0` removes its addresses; `ip route flush` removes explicit
routes and configured gateways, while address-derived connected routes remain.
`-4`, `-br`, `-o`, and `-s` select IPv4, brief, one-line, and statistics output.

`mgmt0` is a separate OS management interface. Connect it through explicit
management switches or patch panels; bare rack LAN sockets do not join racks.
Data NIC names depend on installed cards; inspect `ip link`.

Public addresses are independent of upstream circuits. Buy an address pool in
the shop or **IP RANGES**; select the range delivery uplink there and use its
WAN/LAN instructions to configure your router interfaces and forward/return routes
on each router through **Routing table…**. Guest `netctl` changes are restricted
to that guest's own interfaces; it cannot configure another device or carrier. See the
[provider network guide](PROVIDER_NETWORK_GUIDE.md) for a complete routed example.

## Persistent network configuration

`ip` changes runtime configuration. To restore settings after `reboot`, write
`/etc/network/interfaces` and start networking. This example creates the file
with `printf`; replace addresses and names with those in your topology.

```sh
printf 'auto lo eth0 mgmt0\niface lo inet loopback\niface eth0 inet static\n address 192.0.2.10/24\n gateway 192.0.2.1\n post-up ip route add 198.51.100.0/24 via 192.0.2.254 dev eth0\niface mgmt0 inet dhcp\n' > /etc/network/interfaces
cat /etc/network/interfaces
systemctl restart networking
ip -br a
ip route
reboot
```

Supported stanza methods are `static`, `dhcp`, `manual`, and `loopback` for `lo`.
Use a CIDR address or an address plus `netmask`. `auto` and `allow-hotplug`
select interfaces for `ifup -a`/networking startup. `dns-nameservers` writes
`/etc/resolv.conf`; `post-up` and `up` execute guest commands after configuration.
DHCP leases require a reachable interface with an explicitly configured
`netctl dhcp` pool and an active broadcast/return path.
An invalid file leaves the running network configuration intact.

```sh
ifdown eth0
ifup eth0
ifup -a
systemctl status networking
journalctl -u networking
```

Files, services, addresses, and routes persist in game saves. `reboot` clears
runtime addresses/routes and reapplies the enabled networking service. Files
remain. Normal game power toggles retain saved configuration.

## Shell and files

Single/double quotes, escaped characters, `$NAME`, `${NAME}`, `$?`, pipes, input
and output redirection, append, `;`, `&&`, and `||` are supported.

```sh
mkdir -p /root/config
cd /root/config
export IFACE=eth0
ip -br a | grep "$IFACE" > interface.txt
cat interface.txt
echo '10.0.0.20 database.internal' >> /etc/hosts
getent hosts database.internal
ping database.internal
false && echo skipped || echo recovered
printf '#!/bin/sh\nip addr add 10.0.0.10/16 dev eth0\n' > setup.sh
chmod 755 setup.sh
./setup.sh
```

File commands include `pwd`, `cd`, `ls [-la]`, `mkdir [-p]`, `touch`, `cat`,
`cp`, `mv`, `rm [-rf]`, `rmdir`, `chmod` with octal modes, `find [-name PATTERN]`,
`head`/`tail [-n COUNT]`, `grep [-inv]` with literal patterns, `wc [-lwc]`,
`sort [-ru]`, `uniq`, `tee [-a]`, `df`, and `du`. `cp`/`mv` currently handle files.
`printf` supports `%s`, `%d`, and `%%`. `sh FILE`, `bash -c 'COMMAND'`, `source FILE`,
and executable guest scripts run shell commands; scripts stop at the first error.
Full-screen editors and shell loops/functions are not implemented.

Name lookup uses `/etc/hosts`, literal IPs, and simulated server hostnames.
`getent hosts`, `nslookup`, and `dig` inspect that lookup; they do not query
external DNS servers. Resolver addresses can be stored for configuration.

## Services and SSH

```sh
systemctl list-units
systemctl status ssh
systemctl stop ssh
systemctl enable --now ssh
systemctl is-active ssh
service ssh restart
journalctl -u ssh
ss -lntp
ps aux
ssh root@10.0.0.20
hostnamectl
exit
```

`ssh` connects by management IP through the source server's `mgmt0`. Stopped
SSH services refuse new connections. Connected commands, guest files, and prompts
belong to the remote device. `exit` closes the session. Switch/router sessions
use their IOS console; switch management IPs are configured with
`management ip ADDRESS` in global configuration mode. SSH sessions and listeners
are simulated; there is no authentication or real TCP/SSH implementation.

`systemctl` supports status, start/stop/restart/reload, enable/disable (`--now`),
is-active/is-enabled (`--quiet`), list-units, and list-unit-files. Available units
are `ssh`, `networking`, and `systemd-resolved`. `reboot` restores enabled service
state. `shutdown`/`poweroff` turn off the device.

Hardware commands include `lscpu`, `free -h`, `lsblk`, `smartctl -a /dev/sda`,
`ethtool [-i] IFACE`, `netstat -i`, `uname -a`, `hostname`, and `hostnamectl`.
Use Up/Down for history, Tab for command completion, Clear output or `clear`,
and Paste commands for a sequence of console lines.

Tab suggestions come from the active command object and its current arguments.
For example, `ping -I` suggests interfaces, `systemctl restart` suggests units,
`cat` suggests guest files, and `ip route ... dev` suggests NIC names. Suggestions
also work after pipes and conditionals and inside unfinished quoted paths.
