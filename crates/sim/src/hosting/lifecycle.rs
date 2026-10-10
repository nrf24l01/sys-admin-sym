use super::*;

impl NetworkSim {
    /// Older saves keep the contract on the VM; destruction captures it before
    /// removing anything. No migration creates guests, payments or new leases.
    pub fn hosting_service_provisioning(&self, service: u64) -> Option<HostingServiceProvisioning> {
        let s = self.hosting.services.get(&service)?;
        s.provisioning.clone().or_else(|| {
            let vm = self.hosting.vms.get(&s.vm?)?;
            Some(HostingServiceProvisioning {
                plan: vm.plan.clone(),
                host: vm.host,
                ips: vm.ips.clone(),
                ip_ranges: vm.ip_ranges.clone(),
            })
        })
    }

    /// Destroy the guest and its disk, preserving the paid service and its IPs.
    /// Repeated requests target the service identity, so they cannot delete a
    /// different service or create duplicate billing records.
    pub fn destroy_hosting_vm(&mut self, service: u64) -> Result<(), String> {
        let s = self
            .hosting
            .services
            .get(&service)
            .ok_or("Unknown service")?;
        let Some(vm) = s.vm else { return Ok(()) };
        self.check_vm_management(vm)?;
        let lease = self
            .hosting_service_provisioning(service)
            .ok_or("Missing service contract")?;
        let traffic = self.hosting.vms[&vm].metrics.traffic_bytes;
        let s = self.hosting.services.get_mut(&service).unwrap();
        s.provisioning = Some(lease);
        s.retained_traffic_bytes = traffic;
        s.vm = None;
        self.remove_hosting_vm(vm);
        self.topology_revision += 1;
        self.routing_revision += 1;
        self.refresh_hosting_usage();
        Ok(())
    }

    pub fn hosting_recreation_availability(&self, service: u64) -> Result<DeviceId, String> {
        let s = self
            .hosting
            .services
            .get(&service)
            .ok_or("Unknown service")?;
        if s.terminated_at_ms.is_some() || s.expires_at_ms <= self.hosting.now_ms {
            return Err("Service is terminated or its prepaid period has expired".into());
        }
        if let Some(vm) = s.vm {
            return self
                .hosting
                .vms
                .get(&vm)
                .map(|vm| vm.host)
                .ok_or("Missing VM".into());
        }
        let lease = s.provisioning.as_ref().ok_or("Missing service contract")?;
        self.hosting_resource_placement(&lease.plan, Some((service, lease)))
            .map(|p| p.host)
    }

    /// Validate capacity, management and the retained addresses before mutation.
    /// A product being unpublished/deleted or its web site being down does not
    /// prevent recovery of an already paid service on its original node.
    pub fn recreate_hosting_vm(&mut self, service: u64) -> Result<u64, String> {
        self.hosting_recreation_availability(service)?;
        let s = &self.hosting.services[&service];
        if let Some(vm) = s.vm {
            return Ok(vm);
        }
        let lease = s.provisioning.clone().unwrap();
        let customer = s.customer;
        let expires = s.expires_at_ms;
        let traffic = s.retained_traffic_bytes;
        let place = self.hosting_resource_placement(&lease.plan, Some((service, &lease)))?;
        let id = self.hosting.id();
        let mut vm = self.create_hosting_vm(id, customer, service, &lease.plan, expires, place);
        vm.metrics.traffic_bytes = traffic;
        self.hosting.vms.insert(id, vm);
        self.hosting.services.get_mut(&service).unwrap().vm = Some(id);
        self.topology_revision += 1;
        self.routing_revision += 1;
        self.refresh_hosting_usage();
        Ok(id)
    }

    /// Prorate only the current prepaid period, rounded down to a whole cent.
    pub fn hosting_refund_cents(&self, service: u64) -> Result<u64, String> {
        let s = self
            .hosting
            .services
            .get(&service)
            .ok_or("Unknown service")?;
        if s.terminated_at_ms.is_some() {
            return Ok(0);
        }
        let Some(paid) = self
            .hosting
            .payments
            .iter()
            .rev()
            .find(|p| p.service == service)
        else {
            return Ok(0);
        };
        let remaining = s
            .expires_at_ms
            .saturating_sub(self.hosting.now_ms.max(paid.at_ms));
        let duration = s.expires_at_ms.saturating_sub(paid.at_ms);
        if duration == 0 {
            return Ok(0);
        }
        Ok((u128::from(paid.amount_cents) * u128::from(remaining) / u128::from(duration)) as u64)
    }

    /// Permanent paid-service cancellation is a control-plane cleanup, also
    /// available when a node is offline. It releases leases and records exactly
    /// one refund; expired/customer-cancelled services use termination directly.
    pub fn refund_hosting_service(&mut self, service: u64) -> Result<u64, String> {
        if let Some(refund) = self.hosting.refunds.iter().find(|r| r.service == service) {
            return Ok(refund.amount_cents);
        }
        let amount = self.hosting_refund_cents(service)?;
        if self.hosting.services[&service].terminated_at_ms.is_some() {
            return Ok(0);
        }
        self.terminate_hosting_service(service)?;
        self.money -= (amount / 100) as i64;
        let cents = amount % 100;
        if self.hosting.revenue_remainder_cents < cents {
            self.money -= 1;
            self.hosting.revenue_remainder_cents += 100;
        }
        self.hosting.revenue_remainder_cents -= cents;
        self.hosting.refunds.push(HostingRefund {
            service,
            at_ms: self.hosting.now_ms,
            amount_cents: amount,
        });
        Ok(amount)
    }

    pub(super) fn remove_hosting_vm(&mut self, vm: u64) {
        if let Some(vm) = self.hosting.vms.remove(&vm) {
            for port in vm.interfaces {
                self.remove_host_bridge(port);
                self.provider_remove_virtual(port);
                self.ports.remove(&port);
            }
        }
    }
}
