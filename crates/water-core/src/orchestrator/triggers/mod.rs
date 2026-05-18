pub mod block_anchored_drift;
pub mod scene_flow_dip;
pub mod topic_drift;

use super::Trigger;

#[must_use]
pub fn builtin_triggers() -> Vec<Box<dyn Trigger>> {
    vec![
        Box::new(block_anchored_drift::BlockAnchoredDrift),
        Box::new(scene_flow_dip::SceneFlowDip),
        Box::new(topic_drift::TopicDrift),
        // Remaining 7 in Task 12.
    ]
}
