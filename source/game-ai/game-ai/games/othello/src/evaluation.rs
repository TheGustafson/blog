use crate::Position;
use std::fmt;
use std::str::FromStr;

const CORNERS: u64 = 0x8100_0000_0000_0081;
const CORNER_DANGER: u64 = 0x42c3_0000_0000_c342;

/// Cumulative evaluator stages used by the controlled browser experiment.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum EvaluationProfile {
    Material,
    Mobility,
    Corners,
    Frontier,
    #[default]
    Phase,
}

impl EvaluationProfile {
    pub const ALL: [Self; 5] = [
        Self::Material,
        Self::Mobility,
        Self::Corners,
        Self::Frontier,
        Self::Phase,
    ];

    pub const fn name(self) -> &'static str {
        match self {
            Self::Material => "material",
            Self::Mobility => "mobility",
            Self::Corners => "corners",
            Self::Frontier => "frontier",
            Self::Phase => "phase",
        }
    }
}

impl fmt::Display for EvaluationProfile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

impl FromStr for EvaluationProfile {
    type Err = &'static str;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.to_ascii_lowercase().as_str() {
            "material" | "discs" => Ok(Self::Material),
            "mobility" => Ok(Self::Mobility),
            "corners" => Ok(Self::Corners),
            "frontier" => Ok(Self::Frontier),
            "phase" | "full" => Ok(Self::Phase),
            _ => Err("evaluator must be material, mobility, corners, frontier, or phase"),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct EvaluationWeights {
    pub material: i16,
    pub mobility: i16,
    pub potential_mobility: i16,
    pub corners: i16,
    pub corner_danger: i16,
    pub frontier: i16,
}

/// Traceable position evaluation from the current side's perspective.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Evaluation {
    pub profile: EvaluationProfile,
    pub phase: u8,
    pub material: i16,
    pub mobility: i16,
    pub potential_mobility: i16,
    pub corners: i16,
    pub corner_danger: i16,
    pub frontier: i16,
    pub weights: EvaluationWeights,
    pub total: i32,
}

impl Evaluation {
    pub fn terms_sum(self) -> i32 {
        i32::from(self.material) * i32::from(self.weights.material)
            + i32::from(self.mobility) * i32::from(self.weights.mobility)
            + i32::from(self.potential_mobility) * i32::from(self.weights.potential_mobility)
            + i32::from(self.corners) * i32::from(self.weights.corners)
            + i32::from(self.corner_danger) * i32::from(self.weights.corner_danger)
            + i32::from(self.frontier) * i32::from(self.weights.frontier)
    }
}

/// Evaluates `position` from its side-to-move perspective.
pub fn evaluate(position: Position, profile: EvaluationProfile) -> Evaluation {
    let us = position.side_to_move();
    let them = us.other();
    let phase = position.occupied_count().saturating_sub(4);
    let material = difference(position.disc_count(us), position.disc_count(them));
    let mobility = difference(
        position.legal_placements_for(us).count_ones() as u8,
        position.legal_placements_for(them).count_ones() as u8,
    );
    let potential_mobility = difference(
        position.potential_mobility_bits(us).count_ones() as u8,
        position.potential_mobility_bits(them).count_ones() as u8,
    );
    let corners = difference(
        (position.bits(us) & CORNERS).count_ones() as u8,
        (position.bits(them) & CORNERS).count_ones() as u8,
    );
    let corner_danger = difference(
        (position.bits(us) & CORNER_DANGER).count_ones() as u8,
        (position.bits(them) & CORNER_DANGER).count_ones() as u8,
    );
    let frontier = difference(
        position.frontier_bits(us).count_ones() as u8,
        position.frontier_bits(them).count_ones() as u8,
    );
    let weights = weights(profile, phase);
    let mut evaluation = Evaluation {
        profile,
        phase,
        material,
        mobility,
        potential_mobility,
        corners,
        corner_danger,
        frontier,
        weights,
        total: 0,
    };
    evaluation.total = evaluation.terms_sum();
    evaluation
}

fn weights(profile: EvaluationProfile, phase: u8) -> EvaluationWeights {
    match profile {
        EvaluationProfile::Material => EvaluationWeights {
            material: 10,
            ..EvaluationWeights::default()
        },
        EvaluationProfile::Mobility => EvaluationWeights {
            material: 2,
            mobility: 20,
            potential_mobility: 6,
            ..EvaluationWeights::default()
        },
        EvaluationProfile::Corners => EvaluationWeights {
            material: 2,
            mobility: 20,
            potential_mobility: 6,
            corners: 150,
            corner_danger: -35,
            ..EvaluationWeights::default()
        },
        EvaluationProfile::Frontier => EvaluationWeights {
            material: 2,
            mobility: 20,
            potential_mobility: 6,
            corners: 150,
            corner_danger: -35,
            frontier: -10,
        },
        EvaluationProfile::Phase => EvaluationWeights {
            material: interpolate(1, 14, phase),
            mobility: interpolate(24, 5, phase),
            potential_mobility: interpolate(8, 2, phase),
            corners: 160,
            corner_danger: interpolate(-45, -5, phase),
            frontier: interpolate(-12, -3, phase),
        },
    }
}

fn interpolate(opening: i16, endgame: i16, phase: u8) -> i16 {
    let phase = i32::from(phase.min(60));
    let value = i32::from(opening) * (60 - phase) + i32::from(endgame) * phase;
    (value / 60) as i16
}

const fn difference(ours: u8, theirs: u8) -> i16 {
    ours as i16 - theirs as i16
}
