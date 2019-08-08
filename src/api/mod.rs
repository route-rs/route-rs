use futures::Stream;

mod element;
pub use self::element::*;

mod async_element;
pub use self::async_element::*;

mod classify_element;
pub use self::classify_element::*;

mod join_element;
pub use self::join_element::*;

pub type ElementStream<Input> = Box<dyn Stream<Item = Input, Error = ()> + Send>;
