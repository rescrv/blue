use std::collections::BTreeSet;
use std::sync::RwLock;

#[derive(Default)]
pub struct CompactionsInFlight {
    setsums: RwLock<BTreeSet<[u8; 32]>>,
}

impl CompactionsInFlight {
    pub fn start(&self) -> CompactionInFlight<'_> {
        CompactionInFlight {
            setsums: Vec::new(),
            in_flight: self,
        }
    }
}

pub struct CompactionInFlight<'a> {
    setsums: Vec<[u8; 32]>,
    in_flight: &'a CompactionsInFlight,
}

impl<'a> CompactionInFlight<'a> {
    pub fn add(&mut self, setsum: [u8; 32]) -> bool {
        let mut setsums = self.in_flight.setsums.write().unwrap();
        if !setsums.contains(&setsum) {
            self.setsums.push(setsum);
            setsums.insert(setsum);
            true
        } else {
            false
        }
    }
}

impl<'a> Drop for CompactionInFlight<'a> {
    fn drop(&mut self) {
        let mut setsums = self.in_flight.setsums.write().unwrap();
        for setsum in self.setsums.iter() {
            setsums.remove(setsum);
        }
    }
}
