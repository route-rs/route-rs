pub trait Element {
    type Input: Sized;
    type Output: Sized;

    fn process(&mut self, packet: Self::Input) -> Self::Output;
}

pub trait AsyncElement {
    type Input: Sized;
    type Output: Sized;

    fn process(&mut self, packet: Self::Input) -> Self::Output;
}

pub trait ClassifyElement {
    type Packet: Sized;

    fn classify(&mut self, packet: &Self::Packet) -> usize;
}
