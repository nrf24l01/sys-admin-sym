# Hosting node groups

Players configure a named group once, add virtualization nodes, and select the
group in a hosting product. Members remain synchronized, including nodes added
later. Group membership is live for new orders; existing guests and retained
leases keep their hosts and addresses.

Each node belongs to at most one group. A group owns hypervisor policy (all
overselling ratios, reserves, ballooning, swap and thin provisioning), permitted
IPv4 ranges and optional guest subnet/interface/VLAN mappings. Mappings identify
physical data interfaces by name, resolved to a different port ID on each member.
Physical cabling, individual host addresses and management preparation stay local.

Save or membership changes validate all affected configuration before mutation.
Missing interfaces, non-server members, provider servers, invalid VLANs, unowned
or overlapping subnets, and membership conflicts reject the complete operation.
Grouped nodes cannot override the shared settings. Leaving/deleting a group
retains the last applied configuration and existing guests. Deletion removes
product references; it never converts group placement into unrestricted placement.

Products select zero or more groups alongside existing explicit nodes. Admission,
automatic addressing and market capacity resolve the deduplicated union of current
members. Empty selections cannot be published; empty groups cannot supply orders.
Old saves deserialize with empty groups and group selectors.

UI adds a Groups tab with named drafts, member selection and the shared settings.
Nodes can join, move between or leave groups, and show their group with a shortcut
to its editor. Products show groups with member and ready counts. All new copy has
English and Russian translations. Scroll areas follow the available window size.

Verification covers atomic validation, name-to-port mapping, synchronization,
membership changes, placement, existing leases, old-save compatibility and egui
interaction in both languages. Run the repository's full `make check`.
