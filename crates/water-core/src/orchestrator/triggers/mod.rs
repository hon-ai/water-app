pub mod block_anchored_drift;
pub mod character_dissonance;
pub mod idle_pause_with_present_character;
pub mod no_universe_yet;
pub mod pace_floor;
pub mod scene_flow_dip;
pub mod structural_inflection;
pub mod topic_drift;
pub mod valence_spike;
pub mod world_drift;

use super::Trigger;

#[must_use]
pub fn builtin_triggers() -> Vec<Box<dyn Trigger>> {
    vec![
        Box::new(block_anchored_drift::BlockAnchoredDrift),
        Box::new(scene_flow_dip::SceneFlowDip),
        Box::new(topic_drift::TopicDrift),
        Box::new(valence_spike::ValenceSpike),
        Box::new(structural_inflection::StructuralInflectionTrigger),
        Box::new(pace_floor::PaceFloor),
        Box::new(world_drift::WorldDrift),
        Box::new(no_universe_yet::NoUniverseYet),
        Box::new(character_dissonance::CharacterDissonance),
        Box::new(idle_pause_with_present_character::IdlePauseWithPresentCharacter),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn factory_returns_ten_triggers_with_unique_ids() {
        let triggers = builtin_triggers();
        assert_eq!(triggers.len(), 10);
        let ids: std::collections::HashSet<_> = triggers.iter().map(|t| t.id()).collect();
        assert_eq!(ids.len(), 10);
    }
}
