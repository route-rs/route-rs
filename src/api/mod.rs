use futures::Stream;

pub mod async_element;
pub mod classify_element;
pub mod element;
pub mod join_element;

pub type ElementStream<Input> = Box<dyn Stream<Item = Input, Error = ()> + Send>;
