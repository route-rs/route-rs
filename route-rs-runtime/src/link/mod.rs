use futures::Stream;

/// A simple pull based link.  It is pull based in the sense that packets are only fetched on the input
/// when a packet is requested from the output. This link does not have the abilty store packets internally,
/// so all packets that enter either immediatly leave or are dropped, as dictated by the element. Both sides of
/// this link are on the same thread, hence the label synchronous.
mod sync_link;
pub use self::sync_link::*;

/// Input packets are placed into an intermediate channel that are pulled from the output asynchronously.
/// Asynchronous in that a packets may enter and leave this link asynchronously to each other.  This link is
/// useful for creating queues in the router, buffering, and creating `Task` boundries that can be processed on
/// different threads, or even different cores. Before packets are placed into the queue to be output, they are run
/// through the element defined process function, often performing some sort of transformation.
mod async_link;
pub use self::async_link::*;

/// Uses element defined classifications to sort input into different channels, a good example would
/// be a flow that splits IPv4 and IPv6 packets, asynchronous.
mod classify_link;
pub use self::classify_link::*;

/// Fairly combines all inputs into a single output, asynchronous.
mod join_link;
pub use self::join_link::*;

/// Copies all input to each of its outputs, asynchronous.
mod tee_link;
pub use self::tee_link::*;

/// Drops all packets that are ingressed, asynchronous.
mod blackhole_link;
pub use self::blackhole_link::*;

/// The type that all links take as input, or provide as output. This allows links to be combinatorial
pub type PacketStream<Input> = Box<dyn Stream<Item = Input, Error = ()> + Send>;

/// Task Park is a structure for tasks to place their task handles when sleeping, and where they can
/// check for other tasks that need to be awoken.  As an example, the ingressor and egressor side of
/// an `async_link` both may attempt to sleep when they are unable to work because they are waiting on
/// an action from the other side of the link. Generally, this occurs when the channel joining the `ingressor`
/// and `egressor` encounter a full or empty channel, respectively. They can place their task handle in the `task_park`
/// and expect that when the blocker has been cleared, the other side of the link will awaken them by calling `task.notify()`.
/// `task_park` also contains logic to prevent one side from sleeping when the other side will be unable to awaken them,
/// in order to prevent deadlocks.
mod task_park;
