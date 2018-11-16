
extern crate rand;
extern crate stopwatch;
extern crate pretty_env_logger;

use smartpool::prelude::*;
use self::rand::prelude::*;
use self::rand::XorShiftRng;
use self::behavior::*;
use atomic::{Atomic, Ordering};
use self::stopwatch::Stopwatch;

mod behavior {
    use std::sync::Arc;
    use atomic::Atomic;
    use manhattan_tree::space::*;
    use smartpool::prelude::setup::*;
    use super::rand::prelude::*;
    use super::super::SpatialChannel;

    pub struct MainPool {
        pub focus: Arc<Atomic<[u64; 3]>>,
        followup: MultiChannel<VecDequeChannel>,
        pub spatial: MultiChannel<SpatialChannel<U56Space>>
    }
    impl MainPool {
        pub fn new(rand: &mut impl Rng) -> Self {
            let focus = Arc::new(Atomic::new([
                rand.gen::<u64>() / 16,
                rand.gen::<u64>() / 16,
                rand.gen::<u64>() / 16,
            ]));
            MainPool {
                focus: focus.clone(),
                followup: MultiChannel::new(16, VecDequeChannel::new),
                spatial: MultiChannel::new(16, || SpatialChannel::new(focus.clone(), U56Space)),
            }
        }
    }
    impl PoolBehavior for MainPool {
        type ChannelKey = ();

        fn config(&mut self) -> PoolConfig<Self> {
            PoolConfig {
                threads: 1,
                schedule: ScheduleAlgorithm::HighestFirst,
                levels: vec![vec![ChannelParams {
                    key: (),
                    complete_on_close: false
                }]]
            }
        }

        fn touch_channel<O>(&self, _: <Self as PoolBehavior>::ChannelKey, mut toucher: impl ChannelToucher<O>) -> O {
            toucher.touch(&self.spatial)
        }

        fn touch_channel_mut<O>(&mut self, _: <Self as PoolBehavior>::ChannelKey, mut toucher: impl ChannelToucherMut<O>) -> O {
            toucher.touch_mut(&mut self.spatial)
        }

        fn followup(&self, _: <Self as PoolBehavior>::ChannelKey, task: RunningTask) {
            self.followup.submit(task);
        }
    }
}

const BATCHES: usize = 100;
const BATCH_SIZE: usize = 1000;

#[test]
fn test() {
    pretty_env_logger::init();

    let seed = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15];
    let mut rng = XorShiftRng::from_seed(seed);
    let pool = OwnedPool::new(MainPool::new(&mut rng)).unwrap();
    let atomic = Atomic::new(0usize);
    let timer = Stopwatch::start_new();
    for batch in 0..BATCHES {
        scoped(|scope| {
            for _ in 0..BATCH_SIZE {
                let op = scope.work(|| {
                    atomic.fetch_add(1, Ordering::SeqCst);
                });
                let pos = [
                    rng.gen::<u64>() / 16,
                    rng.gen::<u64>() / 16,
                    rng.gen::<u64>() / 16,
                ];
                pool.pool.spatial.exec(op, pos);
            }
        });
        info!("did batch {}", batch);
    }
    let elapsed = timer.elapsed().as_nanos();
    let average = elapsed as f64 / (BATCHES * BATCH_SIZE) as f64;
    assert_eq!(atomic.load(Ordering::Acquire), BATCHES * BATCH_SIZE);
    info!("average operation took {} ns", average);
}