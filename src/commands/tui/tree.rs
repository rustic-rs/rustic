#[derive(PartialEq, Eq)]
pub struct TreeNode<Data, LeafData> {
    pub data: Data,
    pub open: bool,
    pub children: Vec<Tree<Data, LeafData>>,
}

#[derive(PartialEq, Eq)]
pub enum Tree<Data, LeafData> {
    Node(TreeNode<Data, LeafData>),
    Leaf(LeafData),
}

impl<Data, LeafData> Tree<Data, LeafData> {
    pub fn leaf(data: LeafData) -> Self {
        Self::Leaf(data)
    }
    pub fn node(data: Data, open: bool, children: Vec<Self>) -> Self {
        Self::Node(TreeNode {
            data,
            open,
            children,
        })
    }

    pub fn child_count(&self) -> usize {
        match self {
            Self::Leaf(_) => 0,
            Self::Node(TreeNode { children, .. }) => {
                children.len() + children.iter().map(Self::child_count).sum::<usize>()
            }
        }
    }

    pub fn leaf_data(&self) -> Option<&LeafData> {
        match self {
            Self::Node(_) => None,
            Self::Leaf(data) => Some(data),
        }
    }

    pub fn openable(&self) -> bool {
        matches!(self, Self::Node(node) if !node.open)
    }

    pub fn open(&mut self) {
        if let Self::Node(node) = self {
            node.open = true;
        }
    }
    pub fn close(&mut self) {
        if let Self::Node(node) = self {
            node.open = false;
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = TreeIterItem<'_, Data, LeafData>> {
        TreeIter {
            tree: Some(self),
            iter_stack: Vec::new(),
            only_open: false,
        }
    }

    // iter open tree descending only into open nodes.
    // Note: This iterator skips the root node!
    pub fn iter_open(&self) -> impl Iterator<Item = TreeIterItem<'_, Data, LeafData>> {
        TreeIter {
            tree: Some(self),
            iter_stack: Vec::new(),
            only_open: true,
        }
        .skip(1)
    }

    pub fn nth_mut(&mut self, n: usize) -> Option<&mut Self> {
        let mut count = 0;
        let mut tree = Some(self);
        let mut iter_stack = Vec::new();
        loop {
            if count == n + 1 {
                return tree;
            }
            let item = tree?;
            if let Self::Node(node) = item {
                if node.open {
                    iter_stack.push(node.children.iter_mut());
                }
            }
            tree = next_from_iter_stack(&mut iter_stack);
            count += 1;
        }
    }
}

pub struct TreeIterItem<'a, Data, LeadData> {
    pub depth: usize,
    pub tree: &'a Tree<Data, LeadData>,
}

impl<'a, Data, LeafData> TreeIterItem<'a, Data, LeafData> {
    pub fn leaf_data(&self) -> Option<&LeafData> {
        self.tree.leaf_data()
    }
}

pub struct TreeIter<'a, Data, LeafData> {
    tree: Option<&'a Tree<Data, LeafData>>,
    iter_stack: Vec<std::slice::Iter<'a, Tree<Data, LeafData>>>,
    only_open: bool,
}

impl<'a, Data, LeafData> Iterator for TreeIter<'a, Data, LeafData> {
    type Item = TreeIterItem<'a, Data, LeafData>;
    fn next(&mut self) -> Option<Self::Item> {
        let item = self.tree?;
        let depth = self.iter_stack.len();
        if let Tree::Node(node) = item {
            if !self.only_open || node.open {
                self.iter_stack.push(node.children.iter());
            }
        }

        self.tree = next_from_iter_stack(&mut self.iter_stack);
        Some(TreeIterItem { depth, tree: item })
    }
}

// helper function to get next item from iteration stack when iterating over a Tree
fn next_from_iter_stack<T>(stack: &mut Vec<impl Iterator<Item = T>>) -> Option<T> {
    loop {
        match stack.pop() {
            None => {
                break None;
            }
            Some(mut iter) => {
                if let Some(next) = iter.next() {
                    stack.push(iter);
                    break Some(next);
                }
            }
        }
    }
}
