use futures::Stream;

mod sync_link;
pub use self::sync_link::*;

mod async_link;
pub use self::async_link::*;

mod classify_link;
pub use self::classify_link::*;

mod join_link;
pub use self::join_link::*;

pub type PacketStream<Input> = Box<dyn Stream<Item = Input, Error = ()> + Send>;

mod task_park;
