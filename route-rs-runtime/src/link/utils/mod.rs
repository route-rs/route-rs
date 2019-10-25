/// Task Park is a structure for tasks to place their task handles when sleeping, and where they can
/// check for other tasks that need to be awoken.  As an example, the ingressor and egressor side of
/// an `queue_link` both may attempt to sleep when they are unable to work because they are waiting on
/// an action from the other side of the link. Generally, this occurs when the channel joining the `ingressor`
/// and `egressor` encounter a full or empty channel, respectively. They can place their task handle in the `task_park`
/// and expect that when the blocker has been cleared, the other side of the link will awaken them by calling `task.notify()`.
/// `task_park` also contains logic to prevent one side from sleeping when the other side will be unable to awaken them,
/// in order to prevent deadlocks.
pub mod task_park;
