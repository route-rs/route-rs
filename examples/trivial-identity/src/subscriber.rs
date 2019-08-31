use std::fmt::Debug;
use std::sync::atomic::{AtomicUsize, Ordering};
use tracing::event::Event;
use tracing::field::{Field, Visit};
use tracing::{span, Id, Metadata, Subscriber};

pub struct TrivialIdentitySubscriber {
    ids: AtomicUsize,
}

impl TrivialIdentitySubscriber {
    pub fn new() -> Self {
        TrivialIdentitySubscriber {
            ids: AtomicUsize::new(1),
        }
    }
}

// https://docs.rs/tracing/0.1.7/tracing/subscriber/trait.Subscriber.html
impl Subscriber for TrivialIdentitySubscriber {
    // We're interested in every event, regardless of metadata
    fn enabled(&self, _metadata: &Metadata) -> bool {
        true
    }

    // Set Ids for new spans to be based on their initial attributes/metadata
    fn new_span(&self, span: &span::Attributes) -> Id {
        // record the new span attributes with our visitor
        println!("recording new span");
        span.record(&mut PrintDebugVisitor::new());

        // Return an incrementing ID with every new span
        let id = self.ids.fetch_add(1, Ordering::SeqCst);
        Id::from_u64(id as u64)
    }

    // What do we do with spans? Our visitor will decide
    fn record(&self, _span: &Id, values: &span::Record) {
        println!("recording span");
        values.record(&mut PrintDebugVisitor::new());
    }

    fn record_follows_from(&self, _span: &Id, _follows: &Id) {}

    // What do we do with events? Our visitor will decide
    fn event(&self, event: &Event) {
        println!("recording event");
        event.record(&mut PrintDebugVisitor::new());
    }

    fn enter(&self, _span: &Id) {}

    fn exit(&self, _span: &Id) {}
}

struct PrintDebugVisitor {}

impl PrintDebugVisitor {
    fn new() -> Self {
        PrintDebugVisitor {}
    }
}

impl Visit for PrintDebugVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn Debug) {
        println!("{:?} {:?}", field.name(), value)
    }
}
