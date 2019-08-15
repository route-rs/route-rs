/// Implement a pipeline in the run method, and then call in the main function of the router
pub trait Runner {
    type Input: Sized;
    type Output: Sized;

    fn run(
        input_channel: crossbeam::Receiver<Self::Input>,
        output_channel: crossbeam::Sender<Self::Output>,
    ) -> ();
}
