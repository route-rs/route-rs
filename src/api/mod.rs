use futures::Stream;

pub mod element;
pub mod async_element;

pub type ElementStream<Input> = Box<dyn Stream<Item = Input, Error = ()> + Send>;
