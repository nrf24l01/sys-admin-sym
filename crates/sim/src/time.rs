use serde::{Deserialize, Serialize};

/// A multiplier of elapsed game time, independent of the hosting calendar scale.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum SimulationSpeed {
    #[default]
    X1,
    X2,
    X3,
    X5,
    X10,
    X50,
    X100,
}

impl SimulationSpeed {
    pub const ALL: [Self; 7] = [
        Self::X1,
        Self::X2,
        Self::X3,
        Self::X5,
        Self::X10,
        Self::X50,
        Self::X100,
    ];

    pub const fn multiplier(self) -> u64 {
        match self {
            Self::X1 => 1,
            Self::X2 => 2,
            Self::X3 => 3,
            Self::X5 => 5,
            Self::X10 => 10,
            Self::X50 => 50,
            Self::X100 => 100,
        }
    }
}
