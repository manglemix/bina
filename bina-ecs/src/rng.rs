use std::sync::{atomic::AtomicUsize, OnceLock};

use crossbeam::queue::ArrayQueue;
use parking_lot::Mutex;
use rand::{rngs::SmallRng, RngCore, SeedableRng};
use rand_core::impls::fill_bytes_via_next;

static RANDOM_BYTES_LEN: AtomicUsize = AtomicUsize::new(256);

static RANDOM: OnceLock<Mutex<SmallRng>> = OnceLock::new();
static RANDOM_BYTES: OnceLock<ArrayQueue<u64>> = OnceLock::new();

/// An extension to `rand`'s `SmallRng` that has better throughput for multithreaded code
///
/// This type costs nothing to instantiate, and generating a `u64` only requires popping
/// from a concurrent queue. If the queue is empty when popping, it will be filled with
/// `u64`s again. The underlying `SmallRng` is only instantiated once using the system
/// entropy during the first usage of `BufferedRng`.
#[derive(Clone, Copy)]
pub struct BufferedRng;

impl BufferedRng {
    /// Sets the size of the buffer of random numbers
    ///
    /// Should be called as early as possible
    ///
    /// # Panics
    /// If `BufferedRNG` has already been used, this function will panic.
    /// As such, there is no way to change the buffer size after the first usage
    pub fn set_buffer_size(size: usize) {
        assert!(!RANDOM_BYTES.get().is_none());
        RANDOM_BYTES_LEN.store(size, std::sync::atomic::Ordering::Relaxed);
    }
}

impl RngCore for BufferedRng {
    fn next_u32(&mut self) -> u32 {
        (self.next_u64() >> 32) as u32
    }

    fn next_u64(&mut self) -> u64 {
        let len = RANDOM_BYTES_LEN.load(std::sync::atomic::Ordering::Relaxed);
        let queue = RANDOM_BYTES.get_or_init(|| ArrayQueue::new(len));
        queue.pop().unwrap_or_else(|| {
            let mut lock = RANDOM
                .get_or_init(|| Mutex::new(SmallRng::from_entropy()))
                .lock();
            for _i in 0..len {
                unsafe { queue.push(lock.next_u64()).unwrap_unchecked() };
            }
            lock.next_u64()
        })
    }

    fn fill_bytes(&mut self, dest: &mut [u8]) {
        fill_bytes_via_next(self, dest);
    }

    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), rand::Error> {
        fill_bytes_via_next(self, dest);
        Ok(())
    }
}
