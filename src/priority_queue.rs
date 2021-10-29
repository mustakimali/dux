pub struct PriorityQueue {
    p: min_max_heap::MinMaxHeap<Item>,
    size: usize,
}

#[derive(PartialEq, Eq)]
struct Item(String, u64);
impl Ord for Item {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.1.cmp(&other.1)
    }
}
impl PartialOrd for Item {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.1.partial_cmp(&other.1)
    }
}

impl Default for PriorityQueue {
    fn default() -> Self {
        Self {
            p: Default::default(),
            size: 20,
        }
    }
}

impl PriorityQueue {
    pub fn new(size: usize) -> Self {
        PriorityQueue {
            size,
            ..Default::default()
        }
    }

    pub fn push(&mut self, item: String, priority: u64) {
        if self.p.len() > self.size && priority < self.p.peek_min().unwrap().1 {
            return;
        }

        // trim the vec if full
        if self.p.len() == self.size {
            // delete the current min
            let _ = self.p.push_pop_min(Item(item, priority));
        } else {
            self.p.push(Item(item, priority))
        }
    }

    pub fn get(&self) -> Vec<(String, u64)> {
        let mut p2 = PriorityQueue::new(self.size);
        for item in &self.p {
            p2.push(item.0.clone(), item.1);
        }
        p2.p.into_vec_desc()
            .iter()
            .map(|f| (f.0.clone(), f.1))
            .collect()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn works() {
        let mut q = PriorityQueue::new(2);

        q.push("Item 1".into(), 10);
        q.push("Item 2".into(), 20);
        q.push("Item 3".into(), 30);
        q.push("Item 4".into(), 11);
        q.push("Item 5".into(), 12);

        let q = q.get();

        assert_eq!("Item 3", q[0].0);
        assert_eq!("Item 2", q[1].0);
    }
}
