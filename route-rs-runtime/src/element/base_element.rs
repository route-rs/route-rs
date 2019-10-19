pub trait Element {
    type Input: Send + Clone;
    type Output: Send + Clone;

    fn process(&mut self, packet: Self::Input) -> Option<Self::Output>;
}
