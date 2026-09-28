use std::{
    collections::{HashMap, VecDeque},
    time::{Duration, Instant},
};
#[derive(Default)]
pub struct Quality {
    samples: VecDeque<Option<u64>>,
    updated: Option<Instant>,
}
impl Quality {
    pub fn record(&mut self, latency: Option<u64>) {
        if self.samples.len() == 8 {
            self.samples.pop_front();
        }
        self.samples.push_back(latency);
        self.updated = Some(Instant::now());
    }
    pub fn fresh(&self) -> bool {
        self.updated
            .is_some_and(|at| at.elapsed() < Duration::from_secs(300))
    }
    fn score(&self) -> u64 {
        if !self.fresh() {
            return u64::MAX / 2;
        }
        let mut success: Vec<_> = self.samples.iter().flatten().copied().collect();
        if success.is_empty() {
            return u64::MAX;
        }
        success.sort_unstable();
        let failures = self.samples.len() - success.len();
        success[success.len() / 2]
            .saturating_add(failures as u64 * 1000 / self.samples.len() as u64)
    }
}
pub fn rank(
    mut candidates: Vec<String>,
    selected: &str,
    quality: &HashMap<String, Quality>,
) -> Vec<String> {
    let score = |id: &str| quality.get(id).map_or(u64::MAX / 2, Quality::score);
    candidates.sort_by_key(|id| (score(id), id != selected));
    if let Some(best) = candidates.first() {
        let current = score(selected);
        let best = score(best);
        if current < u64::MAX / 2
            && current <= best.saturating_add(50).max(best.saturating_mul(120) / 100)
        {
            if let Some(i) = candidates.iter().position(|id| id == selected) {
                candidates.swap(0, i);
            }
        }
    }
    candidates.truncate(8);
    if !candidates.iter().any(|id| id == selected) {
        if candidates.len() == 8 {
            candidates.pop();
        }
        candidates.push(selected.to_owned());
    }
    candidates
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn avoids_switching_for_small_gains_and_penalizes_failures() {
        let mut q = HashMap::new();
        let mut current = Quality::default();
        current.record(Some(110));
        let mut other = Quality::default();
        other.record(Some(90));
        q.insert("current".into(), current);
        q.insert("other".into(), other);
        assert_eq!(
            rank(vec!["other".into(), "current".into()], "current", &q)[0],
            "current"
        );
        q.get_mut("current").unwrap().record(None);
        assert_eq!(
            rank(vec!["other".into(), "current".into()], "current", &q)[0],
            "other"
        );
    }
}
