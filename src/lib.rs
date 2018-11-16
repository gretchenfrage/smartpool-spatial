
#![feature(duration_as_u128)]

extern crate smartpool;
extern crate atomic;
#[cfg(test)]
#[macro_use]
extern crate log;

pub extern crate manhattan_tree;

#[cfg(test)]
mod test;

use std::sync::{Arc, Mutex};

use smartpool::{StatusBit, RunningTask};
use smartpool::channel::{Channel, BitAssigner, NotEnoughBits, ExecParam};
use manhattan_tree::collections::MTreeQueue;
use atomic::{Atomic, Ordering};

pub use manhattan_tree::space::{CoordSpace, ZeroCoord};

/// A priority channel based on the manhattan tree, where each task corresponds
/// to a coordinate in some coordinate space. A shared, atomic coordinate _focus_
/// is stored, which can be shared between multiple SpatialChannels, and modified
/// externally. The SpatialChannel will always execute the task with the lowest
/// manhattan distance to the current value of the focus.
pub struct SpatialChannel<S: CoordSpace> where S::Coord: Copy {
    pub focus: Arc<Atomic<S::Coord>>,
    tree: Mutex<MTreeQueue<RunningTask, S>>,
    bit: StatusBit,
}
impl<S: CoordSpace> SpatialChannel<S> where S::Coord: Copy  {
    pub fn new(focus: Arc<Atomic<S::Coord>>, space: S) -> Self {
        SpatialChannel {
            focus,
            tree: Mutex::new(MTreeQueue::new(space)),
            bit: StatusBit::new(),
        }
    }
}
impl<S: CoordSpace + Default> Default for SpatialChannel<S> where S::Coord: Copy + ZeroCoord {
    fn default() -> Self {
        Self::new(Arc::new(Atomic::new(S::Coord::zero_coord())), S::default())
    }
}
impl<S: CoordSpace> Channel for SpatialChannel<S> where S::Coord: Copy {
    fn assign_bits(&mut self, assigner: &mut BitAssigner) -> Result<(), NotEnoughBits> {
        assigner.assign(&mut self.bit)?;
        self.bit.set(!self.tree.get_mut().unwrap().is_empty());
        Ok(())
    }

    fn poll(&self) -> Option<RunningTask> {
        let mut tree = self.tree.lock().unwrap();
        let future = tree.remove(self.focus.load(Ordering::Relaxed));
        self.bit.set(!tree.is_empty());
        future
    }
}
impl<S: CoordSpace> ExecParam for SpatialChannel<S> where S::Coord: Copy {
    type Param = S::Coord;

    fn submit(&self, task: RunningTask, param: S::Coord) {
        let mut tree = self.tree.lock().unwrap();
        tree.insert(param, task);
        self.bit.set(true);
    }
}