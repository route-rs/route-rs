use crossbeam::atomic::AtomicCell;
use futures::task;
use std::sync::Arc;

/**
The task_park module consists of different utilities needed to manage task waking for concurrent
route-rs processors. These utilities should not be exposed via the Processors API.
*/

/// TaskParkState
///
/// This enum represents the state machine of a task_park; A place where a task can 'park' its
/// task handle, and expect the other task sharing this task_park to wake it up at a later time
/// when there is work to do. A simple example exists in  `AsyncLink`. When the provider attempts
/// to pull a packet from the channel, and finds it empty, it must await more packets
/// before it can make forward progress. So it calls `park_and_notify`, which will awaken any
/// task handle inside in the task_park, and place the task_park in the `Parked(task::Task)` state.
/// It can now got to sleep by returning `Async::NotReady`, knowing that the other task will awaken it
/// in the future.
///
/// The task_park can be in one of four states
/// ###
/// # Dead: This state implies that one side holding the `task_park` has dropped, and so the `task_park`
/// can no longer be relied upon. If the state is dead, then the task attempting to park must self notify.
///
/// # Empty: No task is currently parked in the `task_park`
///
/// # Parked: There is a task handle currently parked in the `task_park`. This is the standard way to put your
/// task handle in the `task_park`
///
/// # IndirectParked: There is an atomic reference to a task handle parked here. This is done when one task wishes to
/// park its task handle in many locations, but only wants to be awoken once. This is used when the provider is asleep,
/// and is awaiting any consumer have provided it work to do. When the consumer enqueues a packet, it will unpark the
/// provider by swapping a `None` into the `IndirectParked` `AtomicCell`. Subsequent consumers who enqueue packets will
/// then only retrieve a `None`, and will not overschedule the provider.
pub enum TaskParkState {
    Dead,
    Empty,
    Parked(task::Task),
    IndirectParked(Arc<AtomicCell<Option<task::Task>>>),
}

/// Swaps in the provided TaskParkState. Notifys any task that it finds currently in the `task_park`
/// Returns `true` if it was able to successfully park the provided task, ie the `task_park` is not dead.
fn swap_and_notify(task_park: &Arc<AtomicCell<TaskParkState>>, swap: TaskParkState) -> bool {
    match task_park.swap(swap) {
        TaskParkState::Dead => {
            task_park.store(TaskParkState::Dead);
            false
        }
        TaskParkState::Empty => true,
        TaskParkState::Parked(task) => {
            task.notify();
            true
        }
        TaskParkState::IndirectParked(task) => {
            if let Some(task) = task.swap(None) {
                task.notify();
            }
            true
        }
    }
}

/// Notifies a task if it resides in the `task_park`
/// Use this when you wish you wake up a task but do not wish to sleep yourself
pub fn unpark_and_notify(task_park: &Arc<AtomicCell<TaskParkState>>) {
    swap_and_notify(task_park, TaskParkState::Empty);
}

/// Notifies a task if it resides in the `task_park`, and
/// then parks the callee task in the `task_park`.
/// Use when you wish to sleep the current task
pub fn park_and_notify(task_park: &Arc<AtomicCell<TaskParkState>>) {
    let task = task::current();
    if !swap_and_notify(task_park, TaskParkState::Parked(task)) {
        task::current().notify();
    }
}

/// Simlar to logic to `park_and_notify`, with the key difference being that it
/// takes a provided Arc of the task handle that we wish to park. This enables the
/// callee to park their task handle in multiple locations without fear of overnotificiation.
/// This is used primarily by the egressor of the JoinLink.
pub fn indirect_park_and_notify(
    task_park: &Arc<AtomicCell<TaskParkState>>,
    task: Arc<AtomicCell<Option<task::Task>>>,
) -> bool {
    swap_and_notify(task_park, TaskParkState::IndirectParked(task))
}

/// Notifies a task if it resides in the `task_park`, and then sets
/// the `TaskParkState` to `Dead`.
/// Use when the callee is dropping and will not be able to awaken tasks
/// parked here in the future.
pub fn die_and_notify(task_park: &Arc<AtomicCell<TaskParkState>>) {
    swap_and_notify(task_park, TaskParkState::Dead);
}