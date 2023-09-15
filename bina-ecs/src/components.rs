use std::future::Future;

use crossbeam::{atomic::AtomicCell, utils::Backoff};
use tokio::sync::oneshot::{channel, error::TryRecvError};

use crate::{
    component::{Component, Processable},
    entity::{Entity, Inaccessible},
    universe::Universe,
};

enum FutureValue<T> {
    Pending(tokio::sync::oneshot::Receiver<T>),
    Done(T),
    Failed,
    Taken,
    Moving,
}

/// Represents a handle to a `Future` that can be checked
/// for completion
///
/// When the `Future` completes and the output is taken,
/// the entity with which this handle is a component of will
/// be deleted!
pub struct WatchedFuture<T: Send + Sync + 'static> {
    value: AtomicCell<FutureValue<T>>,
}

impl<T: Send + Sync + 'static> Component for WatchedFuture<T> {
    fn get_ref<'a>(&'a self) -> Self::Reference<'a> {
        self
    }

    fn flush<E: Entity>(
        &mut self,
        my_entity: crate::entity::EntityReference<Inaccessible<E>>,
        universe: &Universe,
    ) {
        let value =
            std::mem::replace(&mut self.value, AtomicCell::new(FutureValue::Taken)).into_inner();
        match value {
            FutureValue::Pending(mut x) => match x.try_recv() {
                Ok(x) => self.value = AtomicCell::new(FutureValue::Done(x)),
                Err(e) => match e {
                    TryRecvError::Empty => self.value = AtomicCell::new(FutureValue::Pending(x)),
                    TryRecvError::Closed => self.value = AtomicCell::new(FutureValue::Failed),
                },
            },
            FutureValue::Taken => universe.queue_remove_entity(my_entity),
            x => self.value = AtomicCell::new(x),
        }
    }
}

impl<T: Send + Sync + 'static> Processable for WatchedFuture<T> {
    fn process<E: crate::entity::Entity>(
        _component: Self::Reference<'_>,
        _my_entity: crate::entity::EntityReference<E>,
        _universe: &crate::universe::Universe,
    ) {
    }
}

#[derive(Debug)]
pub enum WatchedFutureError {
    Pending,
    FutureFailed,
    Taken,
}

impl<T: Send + Sync + 'static> WatchedFuture<T> {
    pub fn new(fut: impl Future<Output = T> + Send + 'static, universe: &Universe) -> Self {
        let (sender, receiver) = channel();
        let _ = universe.enter_tokio();

        tokio::spawn(async {
            let _ = sender.send(fut.await);
        });

        Self {
            value: AtomicCell::new(FutureValue::Pending(receiver)),
        }
    }

    /// Attempt to get the output of a `Future` if it is done
    ///
    /// If an output was successfully retrieved, the entity with
    /// which this handle is a component of will be deleted when
    /// the process frame ends
    pub fn try_get(&self) -> Result<T, WatchedFutureError> {
        let backoff = Backoff::new();
        loop {
            match self.value.swap(FutureValue::Moving) {
                FutureValue::Pending(x) => {
                    self.value.store(FutureValue::Pending(x));
                    break Err(WatchedFutureError::Pending);
                }
                FutureValue::Done(x) => {
                    self.value.store(FutureValue::Taken);
                    break Ok(x);
                }
                FutureValue::Failed => {
                    self.value.store(FutureValue::Failed);
                    break Err(WatchedFutureError::FutureFailed);
                }
                FutureValue::Taken => {
                    self.value.store(FutureValue::Taken);
                    break Err(WatchedFutureError::Taken);
                }
                FutureValue::Moving => backoff.snooze(),
            }
        }
    }
}
