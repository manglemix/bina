use crossbeam::{channel, queue::SegQueue};
use triomphe::Arc;

/// Represents the type of mutability some resource needs to have
/// for a worker to use it
#[derive(PartialEq, Eq)]
pub enum MutabilityAccess {
    /// Immutable access is required
    ///
    /// The given usize represents the maximum number of threads
    /// allowed to access the resource at any given time
    Immutable(usize),
    /// Mutable access is required
    ///
    /// This limits the number of threads accessing the resource to
    /// exactly one
    Mutable,
}

/// The result of an executed WorkerMessage
pub enum ExecResult<T: WorkerMessage> {
    /// A result that can be returned to the `Worker` handle
    Some(T),
    /// No result, but the task should continue
    None,
    /// The task should stop now
    ///
    /// Keep in mind, if there are multiple tasks running, this
    /// will only stop one. You must keep sending this until they
    /// all stop
    Close,
}

pub trait WorkerMessage: Send + Sized + 'static {
    /// The value that is managed by the worker task(s)
    /// If `MUTABILITY_ACCESS` is set to `Mutable`, and your value is not `Sync`,
    /// consider using `Exclusive` to make it `Sync`
    ///
    /// A WorkerValue that is `()` is valid and will allow for Workers that are "pure",
    /// meaning that these workers operate purely on received messages.
    /// You should first consider if your use-case is better suited for a message style system
    /// like this instead of an entity-component system.
    type WorkerValue: Send + Sync;

    /// The type of mutability access this message needs with the WorkerValue
    const MUTABILITY_ACCESS: MutabilityAccess;

    fn exec(self, _value: &Self::WorkerValue) -> ExecResult<Self> {
        unimplemented!()
    }

    fn exec_mut(self, _value: &mut Self::WorkerValue) -> ExecResult<Self> {
        unimplemented!()
    }
}

/// A Worker is a handle to one or more tasks that manage a single value
///
/// This value could be a network socket or file. Whatever the case, this
/// Worker makes it easy for you to run code that could slow down your process
/// on another thread while still having robust control over it.
pub struct Worker<T: WorkerMessage> {
    sender: channel::Sender<T>,
    receiver: channel::Receiver<T>,
    requeued: SegQueue<T>,
}

impl<T: WorkerMessage> Worker<T> {
    /// Spawns one or more tasks depending on the implementation of `T`
    pub fn spawn(mut value: T::WorkerValue) -> Self {
        let (handle_sender, worker_receiver) = channel::unbounded::<T>();
        let (worker_sender, handler_receiver) = channel::unbounded();

        if let MutabilityAccess::Immutable(n) = T::MUTABILITY_ACCESS {
            let value = Arc::new(value);
            for _i in 0..n {
                let value = value.clone();
                let worker_receiver = worker_receiver.clone();
                let worker_sender = worker_sender.clone();

                rayon::spawn(move || {
                    for msg in worker_receiver {
                        let result = match msg.exec(&value) {
                            ExecResult::Some(x) => x,
                            ExecResult::None => continue,
                            ExecResult::Close => break,
                        };
                        if worker_sender.send(result).is_err() {
                            break;
                        }
                    }
                });
            }
        } else {
            rayon::spawn(move || {
                for msg in worker_receiver {
                    let result = match msg.exec_mut(&mut value) {
                        ExecResult::Some(x) => x,
                        ExecResult::None => continue,
                        ExecResult::Close => break,
                    };
                    if worker_sender.send(result).is_err() {
                        break;
                    }
                }
            });
        }

        Self {
            sender: handle_sender,
            receiver: handler_receiver,
            requeued: SegQueue::default(),
        }
    }

    /// Sends a message to the task(s)
    ///
    /// Returns the message back to the sender if the task(s) has ended
    /// for whatever reason
    pub fn send(&self, msg: T) -> Option<T> {
        self.sender.send(msg).err().map(|x| x.0)
    }

    /// Receives a message from the task
    ///
    /// There is no guarantee that received messages are in order.
    /// If a received message is not what one specific thread needs but could
    /// be of use to another, you can `requeue` the received message
    ///
    /// Returns None if the task(s) has ended for whatever reason
    pub fn recv(&self) -> Option<T> {
        self.receiver.recv().ok().or_else(|| self.requeued.pop())
    }

    /// Requeue the given result so that it can be picked up by another thread
    ///
    /// There is no guarantee that another thread will receive the message before
    /// this thread receives it again. This is generally more reliable when the number
    /// of messages is higher. Consider making every message useful regardless of which
    /// thread receives it
    pub fn requeue(&self, result: T) {
        self.requeued.push(result);
    }
}
