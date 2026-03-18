use crate::order::Order;

pub struct Node {
    pub order: Order,
    pub next: Option<usize>,
    pub prev: Option<usize>,
}

pub struct OrderPool {
    pub nodes: Vec<Node>,
    pub free_list: Vec<usize>,
}

impl OrderPool {
    pub fn new(capacity: usize) -> Self {
        Self {
            nodes: Vec::with_capacity(capacity),
            free_list: Vec::with_capacity(capacity),
        }
    }

    pub fn allocate(&mut self, order: Order) -> usize {
        let new_node = Node {
            order,
            next: None,
            prev: None,
        };

        if let Some(index) = self.free_list.pop() {
            self.nodes[index] = new_node;
            index
        } else {
            let index = self.nodes.len();
            self.nodes.push(new_node);
            index
        }
    }

    pub fn deallocate(&mut self, index: usize) {
        self.free_list.push(index);
    }
}
