//! In brief: The runtime is what takes the basket of computation required by the user, links it together into the desired
//! graph, and preps it to be handed to the Tokio for running.

/// Implement the basic transformations for packets in the router.
pub mod processor;

/// Implement classification functions, helping the router decide to send a packet down route A,B,C..and so on.
pub mod classifier;

/// Wrappers around Processors and Classfiers, and implement all the movement of Packets through the Router.
pub mod link;

/// Structure meant to encapsulate a router as and input and output channel. Used by graphgen.
pub mod pipeline;

/// Utilities for the Runtime. Mostly testing constructs.
pub mod utils;

#[macro_export]
/// unpack_link!: Easy Link egressor destructuring
///
/// This macro helps destructure the egressor vector returned by a call to build_link().
///
/// ```
/// use route_rs_runtime::link::primitive::ForkLink;
/// use route_rs_runtime::link::LinkBuilder;
/// use route_rs_runtime::utils::test::packet_generators::immediate_stream;
/// use route_rs_runtime::unpack_link;
///
/// let fork = ForkLink::new()
///     .num_egressors(3)
///     .ingressor(immediate_stream(vec![1,2,3]))
///     .build_link();
/// unpack_link!(fork => _, [a, b, c]);
///
/// ```
macro_rules! unpack_link {
    ($link:expr => $runnables_collector:ident, [$($y:ident), +]) => {
        let (mut runnables, mut egressors) = { $link };
        $runnables_collector.append(&mut runnables);
        unpack_link!(HELPER egressors, $($y), +);
    };

    ($link:expr => _, [$($y:ident), +]) => {
        let (_, mut egressors) = { $link };
        unpack_link!(HELPER egressors, $($y), +);
    };

    ($link:expr => $runnables_collector:ident, $egressors_collect:ident) => {
        let (mut runnables, $egressors_collect) = { $link };
        $runnables_collector.append(&mut runnables);
    };

    ($link:expr => _, mut $egressors_collect:ident) => {
        let (_, mut $egressors_collect) = { $link };
    };

    (HELPER $vector:ident, $e:ident ) => {
        let $e = $vector.remove(0);
    };
    (HELPER $vector:ident, $e:ident, $($y:ident), +) => {
        let $e = $vector.remove(0);
        unpack_link!(HELPER $vector, $($y), +)
    };
}
