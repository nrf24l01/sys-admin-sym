use cloud_provider_sim::{Command, DEFAULT_HOSTING_TIME_SCALE, NetworkSim, SimulationSpeed};

#[test]
fn default_hosting_clock_is_eight_and_a_half_times_faster_at_every_game_speed() {
    assert_eq!(DEFAULT_HOSTING_TIME_SCALE, 510);
    for speed in SimulationSpeed::ALL {
        let mut sim = NetworkSim::new();
        sim.execute(Command::SetSimulationSpeed(speed)).unwrap();
        sim.advance_time(1000);
        assert_eq!(sim.simulation_time_ms(), 1000 * speed.multiplier());
        assert_eq!(
            sim.hosting.now_ms + sim.hosting.pending_ms,
            510_000 * speed.multiplier(),
        );
    }
}

#[test]
fn legacy_default_hosting_clock_upgrades_once_without_changing_elapsed_time_or_custom_rates() {
    for old_rate in [0, 1, 60, 120] {
        let mut sim = NetworkSim::new();
        sim.hosting.market.time_scale = old_rate;
        sim.hosting.now_ms = 300_000;
        sim.hosting.pending_ms = 123_456;
        let saved = ron::to_string(&sim).unwrap();
        let legacy = saved.replacen("clock_defaults_version:1,", "", 1);
        assert_ne!(legacy, saved);
        let mut restored: NetworkSim = ron::from_str(&legacy).unwrap();
        restored.rebuild_indexes();
        let expected_rate = if old_rate == 60 { 510 } else { old_rate };
        assert_eq!(restored.hosting.market.time_scale, expected_rate);
        assert_eq!(restored.hosting.now_ms, 300_000);
        assert_eq!(restored.hosting.pending_ms, 123_456);
        let upgraded = ron::to_string(&restored.hosting).unwrap();
        restored.rebuild_indexes();
        assert_eq!(ron::to_string(&restored.hosting).unwrap(), upgraded);

        // Choosing the old rate explicitly after upgrading remains valid on reload.
        restored.hosting.market.time_scale = 60;
        let saved = ron::to_string(&restored).unwrap();
        let mut reloaded: NetworkSim = ron::from_str(&saved).unwrap();
        reloaded.rebuild_indexes();
        assert_eq!(reloaded.hosting.market.time_scale, 60);
    }
}

#[test]
fn game_speed_scales_elapsed_time_and_preserves_partial_hosting_ticks() {
    let mut sim = NetworkSim::new();
    sim.hosting.market.time_scale = 1;
    sim.execute(Command::SetSimulationSpeed(SimulationSpeed::X100))
        .unwrap();
    sim.advance_time(1000);
    assert_eq!(sim.simulation_time_ms(), 100_000);
    assert_eq!(sim.hosting.pending_ms, 100_000);
    sim.execute(Command::SetSimulationSpeed(SimulationSpeed::X2))
        .unwrap();
    sim.advance_time(1000);
    assert_eq!(sim.simulation_time_ms(), 102_000);
    assert_eq!(sim.hosting.pending_ms, 102_000);
    sim.execute(Command::SetSimulationSpeed(SimulationSpeed::X1))
        .unwrap();
    sim.advance_time(1000);
    assert_eq!(sim.simulation_time_ms(), 103_000);
    assert_eq!(sim.hosting.pending_ms, 103_000);
}

#[test]
fn game_speed_persists_and_old_saves_default_to_normal_speed() {
    let mut sim = NetworkSim::new();
    sim.execute(Command::SetSimulationSpeed(SimulationSpeed::X50))
        .unwrap();
    let saved = ron::to_string(&sim).unwrap();
    let mut restored: NetworkSim = ron::from_str(&saved).unwrap();
    restored.rebuild_indexes();
    assert_eq!(restored.simulation_speed(), SimulationSpeed::X50);
    restored.advance_time(1);
    assert_eq!(restored.simulation_time_ms(), 50);
    let legacy = saved.replacen("simulation_speed:X50,", "", 1);
    assert_ne!(legacy, saved);
    let mut old: NetworkSim = ron::from_str(&legacy).unwrap();
    old.rebuild_indexes();
    assert_eq!(old.simulation_speed(), SimulationSpeed::X1);
    old.advance_time(1);
    assert_eq!(old.simulation_time_ms(), 1);
}
