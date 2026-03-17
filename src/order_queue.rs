use crate::order_pool::OrderPool;

#[derive(Default)]
pub struct OrderQueue {
    pub head: Option<usize>,
    pub tail: Option<usize>,
}

impl OrderQueue {
    pub fn new() -> Self {
        Self {
            head: None,
            tail: None,
        }
    }

    pub fn push_back(&mut self, pool: &mut OrderPool, node_index: usize) {
        match self.tail {
            Some(tail) => {
                let tail_node = pool.nodes.get_mut(tail).unwrap();
                tail_node.next = Some(node_index);

                let node = pool.nodes.get_mut(node_index).unwrap();
                node.prev = Some(tail);
                node.next = None;

                self.tail = Some(node_index);
            }
            None => {
                let node = pool.nodes.get_mut(node_index).unwrap();

                self.tail = Some(node_index);
                self.head = Some(node_index);
                node.next = None;
                node.prev = None;
            }
        }
    }

    pub fn unlink(&mut self, pool: &mut OrderPool, node_index: usize) {
        let (prev, next) = {
            let node = pool.nodes.get(node_index).unwrap();
            (node.prev, node.next)
        };

        if let Some(p) = prev {
            pool.nodes.get_mut(p).unwrap().next = next;
        } else {
            self.head = next;
        }

        if let Some(n) = next {
            pool.nodes.get_mut(n).unwrap().prev = prev;
        } else {
            self.tail = prev;
        }

        let node = pool.nodes.get_mut(node_index).unwrap();
        node.prev = None;
        node.next = None;
    }
}
