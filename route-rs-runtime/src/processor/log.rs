use crate::processor::Processor;
use std::fmt::Debug;
use std::io::{BufWriter, Write};
use std::marker::PhantomData;

/// Processor that logs incoming packets with Debug information, delimited with newlines.
pub struct Log<A, W: Write> {
    phantom: PhantomData<A>,
    log_writer: BufWriter<W>,
}

impl<A, W: Write> Log<A, W> {
    pub fn new(writer: W) -> Log<A, W> {
        Log {
            phantom: PhantomData,
            log_writer: BufWriter::new(writer),
        }
    }
}

/// "It is critical to call flush before BufWriter<W> is dropped.
/// Though dropping will attempt to flush the the contents of the buffer, any errors that happen in
/// the process of dropping will be ignored. Calling flush ensures that the buffer is empty and thus
/// dropping will not even attempt file operations."
/// https://doc.rust-lang.org/std/io/struct.BufWriter.html
impl<A, W: Write> Drop for Log<A, W> {
    fn drop(&mut self) {
        self.log_writer.flush().unwrap();
    }
}

impl<A: Send + Clone + Debug, W: Write> Processor for Log<A, W> {
    type Input = A;
    type Output = A;

    fn process(&mut self, packet: Self::Input) -> Option<Self::Output> {
        self.log_writer
            .write_all(format!("{:?}\n", packet).as_ref())
            .unwrap();
        Some(packet)
    }
}

// TODO: Find common testing abstraction for logging processors
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::fs::{create_dir_all, remove_file};
    use std::io::Read;
    use std::path::Path;
    use uuid::Uuid;

    fn test_log_processor(packets: Vec<i32>, expected_log: &str) {
        let log_dir = Path::new("test_logs");
        let log_filename = format!("{}.log", Uuid::new_v4());
        let log_path = log_dir.join(log_filename);
        create_dir_all(log_dir).unwrap();
        let log_file = File::create(log_path.clone()).unwrap();

        let mut proc = Log::new(log_file);

        let res_packets: Vec<i32> = packets
            .clone()
            .into_iter()
            .map(|packet| proc.process(packet).unwrap())
            .collect();
        assert_eq!(res_packets, packets); // assert identity

        std::mem::drop(proc); // dropping to flush internal BufWriter

        let mut log_file = File::open(log_path.clone()).unwrap();
        let mut contents = String::new();
        log_file.read_to_string(&mut contents).unwrap();
        assert_eq!(contents, expected_log);
        remove_file(log_path).unwrap();
    }

    #[test]
    fn writes_nothing() {
        test_log_processor(vec![], "");
    }

    #[test]
    fn writes_packet() {
        test_log_processor(vec![10], "10\n");
    }

    #[test]
    fn writes_stream_of_packets() {
        test_log_processor((0..10).collect(), "0\n1\n2\n3\n4\n5\n6\n7\n8\n9\n");
    }

    // TODO: Add tests for other impl Write's like network sockets
}
