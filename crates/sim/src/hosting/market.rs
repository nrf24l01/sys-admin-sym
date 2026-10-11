use super::*;

pub fn customer_requirements(category: CustomerCategory) -> CustomerRequirements {
    let (cpu, ram, disk, iops, network, budget, latency) = match category {
        CustomerCategory::Website => (0.4, 1024.0, 15.0, 300.0, 10.0, 450.0, 200.0),
        CustomerCategory::Developer => (3.0, 4096.0, 60.0, 2000.0, 50.0, 800.0, 150.0),
        CustomerCategory::Ecommerce => (2.0, 4096.0, 50.0, 5000.0, 50.0, 1800.0, 80.0),
        CustomerCategory::GameServer => (3.5, 6144.0, 60.0, 1000.0, 80.0, 1800.0, 40.0),
        CustomerCategory::Saas => (4.0, 8192.0, 100.0, 4000.0, 100.0, 2200.0, 80.0),
        CustomerCategory::Database => (4.0, 16384.0, 200.0, 15000.0, 100.0, 3000.0, 50.0),
        CustomerCategory::FileStorage => (1.0, 2048.0, 800.0, 150.0, 500.0, 1400.0, 300.0),
        CustomerCategory::Backup => (0.5, 1024.0, 1500.0, 100.0, 300.0, 1000.0, 1000.0),
        CustomerCategory::HighPerformance => (8.0, 16384.0, 100.0, 12000.0, 500.0, 4500.0, 30.0),
    };
    CustomerRequirements {
        cpu_mhz: cpu * VCPU_MHZ,
        ram_mb: ram,
        disk_gb: disk,
        iops,
        network_mbps: network,
        budget_cents: budget,
        max_latency_ms: latency,
    }
}

pub(crate) fn customer_profile(
    category: CustomerCategory,
    variation: f64,
    seed: u64,
) -> HostingCustomer {
    let mut requirements = customer_requirements(category);
    let size = 0.8 + variation * 0.4;
    requirements.cpu_mhz *= size;
    requirements.ram_mb *= size;
    requirements.disk_gb *= size;
    requirements.iops *= size;
    requirements.network_mbps *= size;
    requirements.budget_cents *= size;
    HostingCustomer {
        id: 0,
        category,
        requirements,
        price_sensitivity: (0.8 + variation * 0.4)
            * if category == CustomerCategory::Website {
                1.5
            } else {
                1.0
            },
        performance_expectation: 0.75 + variation * 0.2,
        reliability_expectation: 0.95 + variation * 0.049,
        services: Default::default(),
        workload_seed: seed,
        satisfaction: 1.0,
        churn_probability: 0.0,
    }
}

fn weights(category: CustomerCategory) -> [f64; 6] {
    // CPU, RAM, disk capacity, I/O, network and price. Quality and reliability
    // are multiplicative requirements, so low price cannot hide unusable service.
    match category {
        CustomerCategory::Website => [0.08, 0.12, 0.05, 0.08, 0.07, 0.60],
        CustomerCategory::Developer => [0.25, 0.20, 0.10, 0.10, 0.05, 0.30],
        CustomerCategory::Ecommerce => [0.15, 0.20, 0.08, 0.22, 0.10, 0.25],
        CustomerCategory::GameServer => [0.35, 0.15, 0.05, 0.08, 0.22, 0.15],
        CustomerCategory::Saas => [0.25, 0.25, 0.05, 0.15, 0.15, 0.15],
        CustomerCategory::Database => [0.15, 0.30, 0.10, 0.30, 0.05, 0.10],
        CustomerCategory::FileStorage => [0.03, 0.05, 0.35, 0.15, 0.25, 0.17],
        CustomerCategory::Backup => [0.02, 0.03, 0.45, 0.10, 0.15, 0.25],
        CustomerCategory::HighPerformance => [0.40, 0.20, 0.03, 0.20, 0.12, 0.05],
    }
}
impl NetworkSim {
    /// Dimensionless utility relative to a category's competitive-market benchmark.
    /// A score of 1 meets the benchmark; diminishing returns cap unused capacity.
    pub fn hosting_attractiveness(&self, p: &HostingProduct, category: CustomerCategory) -> f64 {
        self.hosting_attractiveness_for_customer(p, &customer_profile(category, 0.5, 0))
    }
    pub fn hosting_attractiveness_for_customer(
        &self,
        p: &HostingProduct,
        customer: &HostingCustomer,
    ) -> f64 {
        let req = &customer.requirements;
        let services: Vec<_> = self
            .hosting
            .services
            .values()
            .filter(|s| s.product == p.id)
            .collect();
        let (quality, reliability) = if services.is_empty() {
            (1.0, 1.0)
        } else {
            (
                services.iter().map(|s| s.observed_quality).sum::<f64>() / services.len() as f64,
                services.iter().map(|s| s.availability).sum::<f64>() / services.len() as f64,
            )
        };
        let actual_iops = self
            .hosting_product_nodes(p)
            .iter()
            .map(|n| self.hosting_capacity(*n).iops)
            .fold(0.0, f64::max)
            .min(f64::from(p.disk_iops));
        let cpu = f64::from(p.vcpu) * VCPU_MHZ * f64::from(p.cpu_limit_percent) / 100.0 * quality;
        let ratios = [
            cpu / req.cpu_mhz,
            p.ram_mb as f64 / req.ram_mb,
            p.disk_gb as f64 / req.disk_gb,
            actual_iops / req.iops,
            f64::from(p.port_mbps) * quality / req.network_mbps,
            // Smooth, benchmark-normalized affordability: 1 at the customer's
            // budget, approaches 2 as price falls and 0 as price rises. Unlike
            // a capped budget/price ratio, every discount improves utility.
            2.0 * req.budget_cents / (req.budget_cents + p.monthly_price_cents.max(1) as f64),
        ];
        let mut factor_weights = weights(customer.category);
        factor_weights[5] *= customer.price_sensitivity;
        let weight_sum: f64 = factor_weights.iter().sum();
        let score = ratios
            .iter()
            .zip(factor_weights.map(|w| w / weight_sum))
            .enumerate()
            .map(|(i, (ratio, weight))| {
                let factor = if i == 5 {
                    *ratio
                } else {
                    ratio.clamp(0.01, 2.0)
                };
                factor.ln() * weight
            })
            .sum::<f64>()
            .exp();
        score
            * quality.powf(1.0 + customer.performance_expectation)
            * reliability.powf(3.0 * customer.reliability_expectation)
            * self.hosting.reputation.clamp(0.01, 1.0)
    }
    pub(crate) fn refresh_hosting_market(&mut self) {
        let products: Vec<_> = self.hosting.products.values().cloned().collect();
        let available: Vec<_> = products
            .iter()
            .map(|p| self.hosting_availability(p.id).is_ok())
            .collect();
        let market_fraction = (1.0
            - self
                .hosting
                .services
                .values()
                .filter(|s| s.terminated_at_ms.is_none())
                .count() as f64
                / f64::from(self.hosting.market.population))
        .max(0.0);
        let mut scores = vec![0.0; products.len()];
        let mut demand = vec![0.0; products.len()];
        for category in CustomerCategory::ALL {
            let utilities: Vec<_> = products
                .iter()
                .map(|p| self.hosting_attractiveness(p, category))
                .collect();
            let denominator = 1.0
                + utilities
                    .iter()
                    .zip(&available)
                    .map(|(u, available)| if *available { u.powi(4) } else { 0.0 })
                    .sum::<f64>();
            for (i, utility) in utilities.iter().enumerate() {
                scores[i] += utility / 9.0;
                if available[i] {
                    demand[i] += market_fraction
                        * self.hosting_site_delivery()
                        * f64::from(self.hosting.market.prospects_per_day)
                        / 9.0
                        * utility.powi(4)
                        / denominator;
                }
            }
        }
        for (i, p) in products.iter().enumerate() {
            let stats = self.hosting.market_stats.entry(p.id).or_default();
            stats.attractiveness = scores[i];
            stats.expected_orders_per_day = demand[i];
        }
    }
    pub(crate) fn tick_hosting_market(&mut self) {
        let market_fraction = (1.0
            - self
                .hosting
                .services
                .values()
                .filter(|s| s.terminated_at_ms.is_none())
                .count() as f64
                / f64::from(self.hosting.market.population))
        .max(0.0);
        self.hosting.prospect_credit += market_fraction
            * self.hosting_site_delivery()
            * f64::from(self.hosting.market.prospects_per_day)
            * HOSTING_TICK_MS as f64
            / 86_400_000.0;
        let prospects = self.hosting.prospect_credit.floor() as u32;
        self.hosting.prospect_credit -= f64::from(prospects);
        // Expected volumes depend on category-specific utility, competition with
        // the benchmark, live inventory and service quality. Customers arrive over time.
        for _ in 0..prospects {
            if self
                .hosting
                .services
                .values()
                .filter(|s| s.terminated_at_ms.is_none())
                .count()
                >= self.hosting.market.population as usize
                || self.hosting.products.values().all(|p| !p.published)
            {
                break;
            }
            let category = CustomerCategory::ALL[(self.hosting.random() * 9.0) as usize];
            let variation = self.hosting.random();
            let profile = customer_profile(category, variation, self.hosting.rng_state);
            let candidates: Vec<_> = self
                .hosting
                .products
                .values()
                .filter(|p| p.published && self.hosting_availability(p.id).is_ok())
                .map(|p| {
                    (
                        p.id,
                        self.hosting_attractiveness_for_customer(p, &profile)
                            .powi(4),
                    )
                })
                .collect();
            let total = 1.0 + candidates.iter().map(|(_, u)| *u).sum::<f64>();
            let mut pick = self.hosting.random() * total;
            for (product, utility) in candidates {
                if pick < utility {
                    let order = (1u64 << 63) | self.hosting.next_order;
                    self.hosting.next_order += 1;
                    let _ = self.purchase_hosting_profile(order, product, category, Some(profile));
                    break;
                }
                pick -= utility;
            }
        }
    }
    pub(crate) fn tick_hosting_subscriptions(&mut self) {
        let expired: Vec<_> = self
            .hosting
            .services
            .values()
            .filter(|s| s.terminated_at_ms.is_none() && s.expires_at_ms <= self.hosting.now_ms)
            .map(|s| s.id)
            .collect();
        for id in expired {
            let service = &self.hosting.services[&id];
            if service.vm.is_none() {
                self.hosting
                    .customers
                    .get_mut(&service.customer)
                    .unwrap()
                    .churn_probability = 1.0;
                let _ = self.terminate_hosting_service(id);
                continue;
            }
            let customer = &self.hosting.customers[&service.customer];
            let shortfall = (customer.performance_expectation - service.satisfaction).max(0.0)
                / customer.performance_expectation;
            let unreliability = (customer.reliability_expectation - service.availability).max(0.0);
            let churn = (shortfall.powi(2) * 1.2 + unreliability * 1.5).clamp(0.0, 1.0);
            self.hosting
                .customers
                .get_mut(&service.customer)
                .unwrap()
                .churn_probability = churn;
            if self.hosting.random() < churn {
                let _ = self.terminate_hosting_service(id);
                continue;
            }
            let months = self.hosting.duration();
            let service = self.hosting.services.get_mut(&id).unwrap();
            let at_ms = service.expires_at_ms;
            service.expires_at_ms += u64::from(months) * MONTH_MS;
            service.renewals += 1;
            service.retained_traffic_bytes = 0;
            let amount = service.monthly_price_cents * u64::from(months);
            if let Some(vm) = service.vm {
                self.hosting.vms.get_mut(&vm).unwrap().expires_at_ms = service.expires_at_ms;
                self.hosting.vms.get_mut(&vm).unwrap().metrics.traffic_bytes = 0;
            }
            let product = service.product;
            self.record_hosting_payment(HostingPayment {
                order: 0,
                service: id,
                at_ms,
                months,
                amount_cents: amount,
                renewal: true,
            });
            self.hosting
                .market_stats
                .entry(product)
                .or_default()
                .renewals += 1;
        }
    }
}
