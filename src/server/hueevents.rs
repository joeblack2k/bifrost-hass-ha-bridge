use std::collections::VecDeque;

use chrono::{DateTime, Utc};
use tokio::sync::broadcast::{Receiver, Sender};

use hue::event::EventBlock;

#[derive(Clone, Debug)]
pub struct HueEventRecord {
    timestamp: DateTime<Utc>,
    index: u32,
    pub block: EventBlock,
}

impl HueEventRecord {
    #[must_use]
    pub fn id(&self) -> String {
        format!("{}:{}", self.timestamp.timestamp(), self.index)
    }
}

#[derive(Clone, Debug)]
pub enum HueEventReplay {
    After(Vec<HueEventRecord>),
    SnapshotRequired { checkpoint: Option<String> },
}

#[derive(Clone, Debug)]
pub struct HueEventStream {
    timestamp: DateTime<Utc>,
    index: u32,
    hue_updates: Sender<HueEventRecord>,
    buffer: VecDeque<HueEventRecord>,
}

impl HueEventStream {
    #[must_use]
    pub fn new(buffer_capacity: usize) -> Self {
        Self {
            timestamp: Utc::now(),
            index: 0,
            hue_updates: Sender::new(buffer_capacity),
            buffer: VecDeque::with_capacity(buffer_capacity),
        }
    }

    fn add_to_buffer(&mut self, record: HueEventRecord) {
        if self.buffer.len() == self.buffer.capacity() {
            self.buffer.pop_front();
            self.buffer.push_back(record);
            debug_assert_eq!(self.buffer.len(), self.buffer.capacity());
        } else {
            self.buffer.push_back(record);
        }
    }

    fn generate_record(&mut self, block: EventBlock) -> HueEventRecord {
        let timestamp = Utc::now();
        if timestamp.timestamp() == self.timestamp.timestamp() {
            self.index += 1;
        } else {
            self.index = 0;
            self.timestamp = timestamp;
        }
        HueEventRecord {
            block,
            timestamp,
            index: self.index,
        }
    }

    #[must_use]
    pub fn events_sent_after_id(&self, id: &str) -> HueEventReplay {
        self.buffer
            .iter()
            .position(|record| record.id() == id)
            .map_or_else(
                || HueEventReplay::SnapshotRequired {
                    checkpoint: self.buffer.back().map(HueEventRecord::id),
                },
                |position| {
                    HueEventReplay::After(self.buffer.iter().skip(position + 1).cloned().collect())
                },
            )
    }

    pub fn hue_event(&mut self, block: EventBlock) {
        let record = self.generate_record(block);
        self.add_to_buffer(record.clone());
        if let Err(err) = self.hue_updates.send(record) {
            log::trace!("Overflow on hue event pipe: {err}");
        }
    }

    #[must_use]
    pub fn subscribe(&self) -> Receiver<HueEventRecord> {
        self.hue_updates.subscribe()
    }
}

#[cfg(test)]
mod tests {
    use tokio::sync::broadcast::error::TryRecvError;

    use super::{HueEventReplay, HueEventStream};
    use hue::event::EventBlock;

    fn add_event(
        stream: &mut HueEventStream,
        receiver: &mut tokio::sync::broadcast::Receiver<super::HueEventRecord>,
    ) -> String {
        stream.hue_event(EventBlock::add(Vec::new()));
        receiver.try_recv().expect("event should be buffered").id()
    }

    #[test]
    fn known_id_returns_exact_suffix() {
        let mut stream = HueEventStream::new(4);
        let mut receiver = stream.subscribe();
        let first = add_event(&mut stream, &mut receiver);
        let second = add_event(&mut stream, &mut receiver);
        let third = add_event(&mut stream, &mut receiver);

        let HueEventReplay::After(events) = stream.events_sent_after_id(&first) else {
            panic!("known id should replay a suffix");
        };
        assert_eq!(
            events
                .iter()
                .map(super::HueEventRecord::id)
                .collect::<Vec<_>>(),
            [second, third]
        );
    }

    #[test]
    fn newest_id_returns_empty_replay() {
        let mut stream = HueEventStream::new(2);
        let mut receiver = stream.subscribe();
        let newest = add_event(&mut stream, &mut receiver);

        assert!(matches!(
            stream.events_sent_after_id(&newest),
            HueEventReplay::After(events) if events.is_empty()
        ));
    }

    #[test]
    fn unknown_id_requires_snapshot_at_newest_checkpoint() {
        let mut stream = HueEventStream::new(2);
        let mut receiver = stream.subscribe();
        let _first = add_event(&mut stream, &mut receiver);
        let newest = add_event(&mut stream, &mut receiver);

        assert!(matches!(
            stream.events_sent_after_id("unknown"),
            HueEventReplay::SnapshotRequired { checkpoint: Some(checkpoint) } if checkpoint == newest
        ));
    }

    #[test]
    fn evicted_id_requires_snapshot_at_newest_checkpoint() {
        let mut stream = HueEventStream::new(2);
        let mut receiver = stream.subscribe();
        let evicted = add_event(&mut stream, &mut receiver);
        let _second = add_event(&mut stream, &mut receiver);
        let newest = add_event(&mut stream, &mut receiver);

        assert!(matches!(
            stream.events_sent_after_id(&evicted),
            HueEventReplay::SnapshotRequired { checkpoint: Some(checkpoint) } if checkpoint == newest
        ));
    }

    #[test]
    fn empty_buffer_has_no_snapshot_checkpoint() {
        let stream = HueEventStream::new(2);

        assert!(matches!(
            stream.events_sent_after_id("unknown"),
            HueEventReplay::SnapshotRequired { checkpoint: None }
        ));
    }

    #[test]
    fn broadcast_capacity_matches_buffer_capacity() {
        let mut stream = HueEventStream::new(2);
        let mut receiver = stream.subscribe();
        for _ in 0..3 {
            stream.hue_event(EventBlock::add(Vec::new()));
        }

        assert!(matches!(receiver.try_recv(), Err(TryRecvError::Lagged(1))));
        assert!(receiver.try_recv().is_ok());
        assert!(receiver.try_recv().is_ok());
        assert!(matches!(receiver.try_recv(), Err(TryRecvError::Empty)));
    }
}
