//! FIFO eviction. Max 2 pills on-screen at once; new candidate evicts the
//! older. Pinned pills do not count.

use super::state::{Pill, PillLifecycle};

pub const MAX_ON_SCREEN: usize = 2;

/// Returns the index of the pill to evict (older of the on-screen pills),
/// or None if there's room.
#[must_use]
pub fn pick_evictee(pills: &[Pill]) -> Option<usize> {
    let on_screen: Vec<(usize, &Pill)> = pills
        .iter()
        .enumerate()
        .filter(|(_, p)| p.state == PillLifecycle::OnScreen)
        .collect();
    if on_screen.len() < MAX_ON_SCREEN {
        return None;
    }
    on_screen
        .iter()
        .min_by_key(|(_, p)| p.created_at)
        .map(|(i, _)| *i)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Id;
    use std::time::{Duration, Instant};

    fn pill_at(t0: Instant, secs: u64, state: PillLifecycle) -> Pill {
        Pill {
            id: Id::new(),
            state,
            created_at: t0 + Duration::from_secs(secs),
            speaker_id: "echo".into(),
            trigger_id: "t".into(),
            text: None,
            block_target_id: None,
            parent_pill_id: None,
        }
    }

    #[test]
    fn no_evictee_when_under_max() {
        let t0 = Instant::now();
        let pills = vec![pill_at(t0, 0, PillLifecycle::OnScreen)];
        assert!(pick_evictee(&pills).is_none());
    }

    #[test]
    fn picks_oldest_on_screen() {
        let t0 = Instant::now();
        let pills = vec![
            pill_at(t0, 0, PillLifecycle::OnScreen),
            pill_at(t0, 30, PillLifecycle::OnScreen),
        ];
        assert_eq!(pick_evictee(&pills), Some(0));
    }

    #[test]
    fn pinned_pills_do_not_count() {
        let t0 = Instant::now();
        let pills = vec![
            pill_at(t0, 0, PillLifecycle::Pinned),
            pill_at(t0, 30, PillLifecycle::OnScreen),
            pill_at(t0, 60, PillLifecycle::OnScreen),
        ];
        assert_eq!(pick_evictee(&pills), Some(1));
    }
}
