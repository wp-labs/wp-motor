pub struct TaskChannel<T> {
    holds: Vec<tokio::sync::mpsc::Sender<T>>,
}

impl<T> Default for TaskChannel<T> {
    fn default() -> Self {
        TaskChannel { holds: Vec::new() }
    }
}

impl<T> TaskChannel<T>
where
    T: Clone,
{
    pub fn channel(
        &mut self,
        count: usize,
    ) -> (tokio::sync::mpsc::Sender<T>, tokio::sync::mpsc::Receiver<T>) {
        let (s, r): (tokio::sync::mpsc::Sender<T>, tokio::sync::mpsc::Receiver<T>) =
            tokio::sync::mpsc::channel(count);
        self.holds.push(s.clone());
        (s, r)
    }
}
