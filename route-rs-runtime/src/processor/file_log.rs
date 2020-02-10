use crate::processor::{Log, Processor};
use std::fmt::Debug;
use std::fs::File;
use std::marker::PhantomData;

/// Wraps Log processor with a simpler File-specific interface
/// You must provide a unique log filename.
pub struct FileLog<A> {
    phantom: PhantomData<A>,
    log_processor: Log<A, File>,
}

impl<A> FileLog<A> {
    pub fn new(name: &str) -> std::io::Result<FileLog<A>> {
        Ok(FileLog {
            phantom: PhantomData,
            log_processor: Log::new(File::create(name)?),
        })
    }
}

impl<A: Send + Clone + Debug> Processor for FileLog<A> {
    type Input = A;
    type Output = A;

    fn process(&mut self, packet: Self::Input) -> Option<Self::Output> {
        self.log_processor.process(packet)
    }
}

// TODO: Find common testing abstraction for logging processors
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{create_dir_all, remove_file};
    use std::io::Read;
    use std::path::Path;
    use uuid::Uuid;

    fn test_file_log_processor(packets: Vec<i32>, expected_log: &str) {
        let log_dir = Path::new("test_logs");
        let log_file = format!("{}.log", Uuid::new_v4());
        let log_path = log_dir.join(log_file);
        create_dir_all(log_dir).unwrap();

        let mut proc = FileLog::new(log_path.to_str().unwrap()).unwrap();

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
        test_file_log_processor(vec![], "");
    }

    #[test]
    fn writes_packet() {
        test_file_log_processor(vec![10], "10\n");
    }

    #[test]
    fn writes_stream_of_packets() {
        test_file_log_processor((0..10).collect(), "0\n1\n2\n3\n4\n5\n6\n7\n8\n9\n");
    }
}
