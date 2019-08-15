use crossbeam::atomic::AtomicCell;
use futures::task;
use std::sync::Arc;

/**
The task_park module consists of different utilities needed to manage task waking for concurrent
route-rs elements. These utilities should not be exposed via the Elements API.
*/

pub enum TaskParkState {
    Dead,
    Empty,
    Full(task::Task),
}

pub fn unpark_and_notify(task_park: &Arc<AtomicCell<TaskParkState>>) {
    match task_park.swap(TaskParkState::Empty) {
        TaskParkState::Full(other_task) => {
            other_task.notify();
        }
        TaskParkState::Empty => {}
        // If the `task_park` died, we must retain
        // the Dead state to prevent deadlocks on Drop.
        TaskParkState::Dead => {
            task_park.store(TaskParkState::Dead);
        }
    }
}

pub fn park_and_notify(task_park: &Arc<AtomicCell<TaskParkState>>) {
    let current_task = task::current();
    match task_park.swap(TaskParkState::Full(current_task)) {
        TaskParkState::Full(other_task) => {
            other_task.notify();
        }
        TaskParkState::Empty => {}
        // If the `task_park` died, we must self notify
        // and retain the Dead state to prevent deadlocks on Drop.
        TaskParkState::Dead => {
            task_park.store(TaskParkState::Dead);
            task::current().notify();
        }
    }
}

pub fn die_and_notify(task_park: &Arc<AtomicCell<TaskParkState>>) {
    if let TaskParkState::Full(other_task) = task_park.swap(TaskParkState::Dead) {
        other_task.notify();
    }
}
