# Install the controller and register a VM node

The controller is the provider's storefront and management service. A VM node
registers with that service over the private network. The controller needs its
own physical server, separate from customer virtualization nodes.

## On the controller

```sh
./setup-main-hoster-server.sh
systemctl status hosting-provider
```

The script installs, enables and starts `hosting-provider` on **this** server.
It uses the server's configured interfaces; it does not assign addresses or
create routes. Re-running the script is safe.

## On the VM node

```sh
./vm-prepare.sh 10.0.10.10
vm-host status
```

Use the controller's **private address**, not its public storefront address and
not the router's gateway. Preparation requires the node to be operational and
its Linux-selected private path to reach the running controller service.

Internal registration can happen before the public storefront is connected.
If you change the controller's private IP or replace the controller, prepare the
node again with the new address. Existing guests remain, but new placement and
management need a valid registration.

Next: [obtain a public range](public-range.md).
