use std::{
    cmp::Reverse,
    collections::{BinaryHeap, HashMap},
};

use bevy::prelude::*;

use crate::coordinates::ChunkPos;

/// Processing stage of a chunk currently inside the streaming window.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChunkLifecycle {
    Requested,
    Generating,
    Generated,
    Meshing,
    Ready,
}

/// Deduplicated generation queue and lifecycle registry for streamed chunks.
#[derive(Debug, Default, Resource)]
pub struct ChunkGenerationQueue {
    states: HashMap<ChunkPos, ChunkLifecycle>,
    pending: BinaryHeap<Reverse<(i64, i64, ChunkPos)>>,
}

impl ChunkGenerationQueue {
    pub fn state(&self, position: ChunkPos) -> Option<ChunkLifecycle> {
        self.states.get(&position).copied()
    }

    pub fn tracked_count(&self) -> usize {
        self.states.len()
    }

    pub fn pending_count(&self) -> usize {
        self.states
            .values()
            .filter(|&&state| state == ChunkLifecycle::Requested)
            .count()
    }

    pub fn generating_count(&self) -> usize {
        self.states
            .values()
            .filter(|&&state| state == ChunkLifecycle::Generating)
            .count()
    }

    pub fn request(&mut self, position: ChunkPos) -> bool {
        if self.states.contains_key(&position) {
            return false;
        }
        self.states.insert(position, ChunkLifecycle::Requested);
        true
    }

    pub(crate) fn cancel(&mut self, position: ChunkPos) -> Option<ChunkLifecycle> {
        self.states.remove(&position)
    }

    pub(crate) fn positions(&self) -> impl Iterator<Item = ChunkPos> + '_ {
        self.states.keys().copied()
    }

    pub(crate) fn register_generated(&mut self, position: ChunkPos) {
        match self.states.get(&position) {
            Some(ChunkLifecycle::Generated | ChunkLifecycle::Meshing | ChunkLifecycle::Ready) => {}
            _ => {
                self.states.insert(position, ChunkLifecycle::Generated);
            }
        }
    }

    pub(crate) fn reprioritize(&mut self, center: ChunkPos) {
        self.pending.clear();
        for (&position, &state) in &self.states {
            if state == ChunkLifecycle::Requested {
                self.pending.push(Reverse(priority(position, center)));
            }
        }
    }

    pub(crate) fn take_next_generation(&mut self) -> Option<ChunkPos> {
        while let Some(Reverse((_, _, position))) = self.pending.pop() {
            if self.transition(
                position,
                &[ChunkLifecycle::Requested],
                ChunkLifecycle::Generating,
            ) {
                return Some(position);
            }
        }
        None
    }

    pub(crate) fn finish_generation(&mut self, position: ChunkPos) -> bool {
        self.transition(
            position,
            &[ChunkLifecycle::Generating],
            ChunkLifecycle::Generated,
        )
    }

    pub(crate) fn begin_meshing(&mut self, position: ChunkPos) -> bool {
        self.transition(
            position,
            &[ChunkLifecycle::Generated, ChunkLifecycle::Ready],
            ChunkLifecycle::Meshing,
        )
    }

    pub(crate) fn finish_meshing(&mut self, position: ChunkPos) -> bool {
        self.transition(position, &[ChunkLifecycle::Meshing], ChunkLifecycle::Ready)
    }

    fn transition(
        &mut self,
        position: ChunkPos,
        allowed: &[ChunkLifecycle],
        next: ChunkLifecycle,
    ) -> bool {
        let Some(state) = self.states.get_mut(&position) else {
            return false;
        };
        if !allowed.contains(state) {
            return false;
        }
        *state = next;
        true
    }
}

fn priority(position: ChunkPos, center: ChunkPos) -> (i64, i64, ChunkPos) {
    let dx = i64::from(position.x) - i64::from(center.x);
    let dy = i64::from(position.y) - i64::from(center.y);
    let dz = i64::from(position.z) - i64::from(center.z);
    let horizontal_squared = dx * dx + dz * dz;
    (horizontal_squared + dy * dy, horizontal_squared, position)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duplicate_requests_create_only_one_pending_job() {
        let position = ChunkPos::new(2, -1, 7);
        let mut queue = ChunkGenerationQueue::default();

        assert!(queue.request(position));
        assert!(!queue.request(position));
        assert_eq!(queue.tracked_count(), 1);
        assert_eq!(queue.pending_count(), 1);

        queue.reprioritize(ChunkPos::default());
        assert_eq!(queue.take_next_generation(), Some(position));
        assert_eq!(queue.take_next_generation(), None);
    }

    #[test]
    fn lifecycle_rejects_invalid_transitions() {
        let position = ChunkPos::default();
        let mut queue = ChunkGenerationQueue::default();
        queue.request(position);

        assert!(!queue.finish_generation(position));
        queue.reprioritize(position);
        assert_eq!(queue.take_next_generation(), Some(position));
        assert_eq!(queue.state(position), Some(ChunkLifecycle::Generating));
        assert!(queue.finish_generation(position));
        assert_eq!(queue.state(position), Some(ChunkLifecycle::Generated));
        assert!(queue.begin_meshing(position));
        assert_eq!(queue.state(position), Some(ChunkLifecycle::Meshing));
        assert!(queue.finish_meshing(position));
        assert_eq!(queue.state(position), Some(ChunkLifecycle::Ready));
        assert!(!queue.finish_generation(position));
    }

    #[test]
    fn nearest_job_is_taken_first() {
        let center = ChunkPos::new(10, 3, -4);
        let near = ChunkPos::new(11, 3, -4);
        let far = ChunkPos::new(15, 3, -4);
        let mut queue = ChunkGenerationQueue::default();
        queue.request(far);
        queue.request(near);
        queue.reprioritize(center);

        assert_eq!(queue.take_next_generation(), Some(near));
        assert!(queue.finish_generation(near));
        assert_eq!(queue.take_next_generation(), Some(far));
    }
}
