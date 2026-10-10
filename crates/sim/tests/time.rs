use cloud_provider_sim::{Command, NetworkSim, SimulationSpeed};

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
