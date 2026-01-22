/// Trait for recording statistical events
///
/// This trait provides methods to track the beginning, end, and completion
/// of tasks or events across different data dimensions.
pub trait StatRecorder<T> {
    /// Records the beginning of an event
    ///
    /// # Arguments
    /// * `target` - The target identifier (e.g., rule name)
    /// * `dat_key` - Data key for dimensional tracking
    fn record_begin(&mut self, target: &str, dat_key: T);

    /// Records the end of an event
    ///
    /// # Arguments
    /// * `target` - The target identifier (e.g., rule name)
    /// * `dat_key` - Data key for dimensional tracking
    fn record_end(&mut self, target: &str, dat_key: T);

    /// Records a complete task (both begin and end)
    ///
    /// # Arguments
    /// * `target` - The target identifier (e.g., rule name)
    /// * `dat_key` - Data key for dimensional tracking
    fn record_task(&mut self, target: &str, dat_key: T);
}
