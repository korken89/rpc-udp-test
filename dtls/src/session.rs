/// Record number tracking.
pub struct RecordNumber {
    // epoch: u16, // Always 0
    sequence_number: u64, // Max u48...
}

impl RecordNumber {
    /// Create a new record number counter.
    pub fn new() -> Self {
        Self { sequence_number: 0 }
    }
}
