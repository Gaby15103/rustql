use crate::storage::{PAGE_SIZE, Pager, ROW_SIZE};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum NodeType {
    Internal = 0,
    Leaf = 1,
}

// Common Node Header Layout (6 bytes)
pub const NODE_TYPE_SIZE: usize = std::mem::size_of::<u8>();
pub const NODE_TYPE_OFFSET: usize = 0;
pub const IS_ROOT_SIZE: usize = std::mem::size_of::<u8>();
pub const IS_ROOT_OFFSET: usize = NODE_TYPE_OFFSET + NODE_TYPE_SIZE;
pub const PARENT_POINTER_SIZE: usize = std::mem::size_of::<u32>();
pub const PARENT_POINTER_OFFSET: usize = IS_ROOT_OFFSET + IS_ROOT_SIZE;
pub const COMMON_NODE_HEADER_SIZE: usize = NODE_TYPE_SIZE + IS_ROOT_SIZE + PARENT_POINTER_SIZE;

// Leaf Node Header Layout (14 bytes total)
pub const LEAF_NODE_NUM_CELLS_SIZE: usize = std::mem::size_of::<u32>();
pub const LEAF_NODE_NUM_CELLS_OFFSET: usize = COMMON_NODE_HEADER_SIZE;
pub const LEAF_NODE_NEXT_LEAF_SIZE: usize = std::mem::size_of::<u32>();
pub const LEAF_NODE_NEXT_LEAF_OFFSET: usize = LEAF_NODE_NUM_CELLS_OFFSET + LEAF_NODE_NUM_CELLS_SIZE;
pub const LEAF_NODE_HEADER_SIZE: usize =
    COMMON_NODE_HEADER_SIZE + LEAF_NODE_NUM_CELLS_SIZE + LEAF_NODE_NEXT_LEAF_SIZE;

// Leaf Node Body Layout
pub const LEAF_NODE_KEY_SIZE: usize = std::mem::size_of::<u32>();
pub const LEAF_NODE_KEY_OFFSET: usize = 0;
pub const LEAF_NODE_VALUE_SIZE: usize = ROW_SIZE;
pub const LEAF_NODE_VALUE_OFFSET: usize = LEAF_NODE_KEY_OFFSET + LEAF_NODE_KEY_SIZE;
pub const LEAF_NODE_CELL_SIZE: usize = LEAF_NODE_KEY_SIZE + LEAF_NODE_VALUE_SIZE;
pub const LEAF_NODE_SPACE_FOR_CELLS: usize = PAGE_SIZE - LEAF_NODE_HEADER_SIZE;
pub const LEAF_NODE_MAX_CELLS: usize = LEAF_NODE_SPACE_FOR_CELLS / LEAF_NODE_CELL_SIZE;

// Split Counts (14 total cells -> 7 right, 7 left)
pub const LEAF_NODE_RIGHT_SPLIT_COUNT: usize = (LEAF_NODE_MAX_CELLS + 1) / 2;
pub const LEAF_NODE_LEFT_SPLIT_COUNT: usize =
    (LEAF_NODE_MAX_CELLS + 1) - LEAF_NODE_RIGHT_SPLIT_COUNT;

// Internal Node Header Layout (14 bytes)
pub const INTERNAL_NODE_NUM_KEYS_SIZE: usize = std::mem::size_of::<u32>();
pub const INTERNAL_NODE_NUM_KEYS_OFFSET: usize = COMMON_NODE_HEADER_SIZE;
pub const INTERNAL_NODE_RIGHT_CHILD_SIZE: usize = std::mem::size_of::<u32>();
pub const INTERNAL_NODE_RIGHT_CHILD_OFFSET: usize =
    INTERNAL_NODE_NUM_KEYS_OFFSET + INTERNAL_NODE_NUM_KEYS_SIZE;
pub const INTERNAL_NODE_HEADER_SIZE: usize =
    COMMON_NODE_HEADER_SIZE + INTERNAL_NODE_NUM_KEYS_SIZE + INTERNAL_NODE_RIGHT_CHILD_SIZE;

// Internal Node Body Layout (Child pointer + Key = 8 bytes per cell)
pub const INTERNAL_NODE_KEY_SIZE: usize = std::mem::size_of::<u32>();
pub const INTERNAL_NODE_CHILD_SIZE: usize = std::mem::size_of::<u32>();
pub const INTERNAL_NODE_CELL_SIZE: usize = INTERNAL_NODE_CHILD_SIZE + INTERNAL_NODE_KEY_SIZE;

pub const INTERNAL_NODE_SPACE_FOR_CELLS: usize = PAGE_SIZE - INTERNAL_NODE_HEADER_SIZE;
pub const INTERNAL_NODE_MAX_CELLS: usize = INTERNAL_NODE_SPACE_FOR_CELLS / INTERNAL_NODE_CELL_SIZE;
pub const INVALID_PAGE_NUM: u32 = u32::MAX;

pub fn get_node_type(page: &[u8]) -> NodeType {
    match page[NODE_TYPE_OFFSET] {
        0 => NodeType::Internal,
        1 => NodeType::Leaf,
        other => panic!("Unknown node type {}", other),
    }
}

pub fn set_node_type(page: &mut [u8], node_type: NodeType) {
    page[NODE_TYPE_OFFSET] = node_type as u8;
}

pub fn is_node_root(page: &[u8]) -> bool {
    page[IS_ROOT_OFFSET] == 1
}

pub fn set_node_root(page: &mut [u8], is_root: bool) {
    page[IS_ROOT_OFFSET] = if is_root { 1 } else { 0 };
}

pub fn get_node_max_key(pager: &mut Pager, page_num: usize) -> u32 {
    let page = pager.get_page(page_num);
    match get_node_type(page.as_ref()) {
        NodeType::Leaf => {
            let leaf = LeafNode::new(page.as_mut());
            let num_cells = leaf.num_cells() as usize;
            leaf.key(num_cells - 1)
        }
        NodeType::Internal => {
            let right_child = InternalNode::new(page.as_mut()).right_child() as usize;
            get_node_max_key(pager, right_child)
        }
    }
}

pub fn node_parent(page: &[u8]) -> u32 {
    let bytes: [u8; 4] = page[PARENT_POINTER_OFFSET..PARENT_POINTER_OFFSET + PARENT_POINTER_SIZE]
        .try_into()
        .unwrap();
    u32::from_ne_bytes(bytes)
}

pub fn set_node_parent(page: &mut [u8], parent_page_num: u32) {
    page[PARENT_POINTER_OFFSET..PARENT_POINTER_OFFSET + PARENT_POINTER_SIZE]
        .copy_from_slice(&parent_page_num.to_ne_bytes());
}

fn update_ancestor_max_keys(
    pager: &mut Pager,
    mut child_page_num: usize,
    old_max: u32,
    new_max: u32,
) {
    if new_max <= old_max {
        return;
    }

    while !is_node_root(pager.get_page(child_page_num).as_ref()) {
        let parent_page_num = node_parent(pager.get_page(child_page_num).as_ref()) as usize;
        let is_right_child = {
            let parent_page = pager.get_page(parent_page_num);
            InternalNode::new(parent_page.as_mut()).right_child() as usize == child_page_num
        };

        if is_right_child {
            // Keep climbing up the right spine until we reach a node that is a keyed child
            child_page_num = parent_page_num;
        } else {
            let parent_page = pager.get_page(parent_page_num);
            InternalNode::new(parent_page.as_mut()).update_key(old_max, new_max);
            break;
        }
    }
}

pub struct LeafNode<'a> {
    pub page: &'a mut [u8],
}

impl<'a> LeafNode<'a> {
    pub fn new(page: &'a mut [u8]) -> Self {
        Self { page }
    }

    pub fn initialize(&mut self) {
        set_node_type(self.page, NodeType::Leaf);
        set_node_root(self.page, false);
        self.set_num_cells(0);
        self.set_next_leaf(0);
    }

    pub fn next_leaf(&self) -> u32 {
        let bytes: [u8; 4] = self.page
            [LEAF_NODE_NEXT_LEAF_OFFSET..LEAF_NODE_NEXT_LEAF_OFFSET + LEAF_NODE_NEXT_LEAF_SIZE]
            .try_into()
            .unwrap();
        u32::from_ne_bytes(bytes)
    }

    pub fn set_next_leaf(&mut self, next_page_num: u32) {
        self.page
            [LEAF_NODE_NEXT_LEAF_OFFSET..LEAF_NODE_NEXT_LEAF_OFFSET + LEAF_NODE_NEXT_LEAF_SIZE]
            .copy_from_slice(&next_page_num.to_ne_bytes());
    }

    pub fn read_num_cells(page: &[u8]) -> u32 {
        let bytes: [u8; 4] = page
            [LEAF_NODE_NUM_CELLS_OFFSET..LEAF_NODE_NUM_CELLS_OFFSET + LEAF_NODE_NUM_CELLS_SIZE]
            .try_into()
            .unwrap();
        u32::from_ne_bytes(bytes)
    }

    pub fn num_cells(&self) -> u32 {
        Self::read_num_cells(self.page)
    }

    pub fn set_num_cells(&mut self, num: u32) {
        self.page
            [LEAF_NODE_NUM_CELLS_OFFSET..LEAF_NODE_NUM_CELLS_OFFSET + LEAF_NODE_NUM_CELLS_SIZE]
            .copy_from_slice(&num.to_ne_bytes());
    }

    fn cell_offset(cell_num: usize) -> usize {
        LEAF_NODE_HEADER_SIZE + cell_num * LEAF_NODE_CELL_SIZE
    }

    pub fn read_key(page: &[u8], cell_num: usize) -> u32 {
        let offset = Self::cell_offset(cell_num) + LEAF_NODE_KEY_OFFSET;
        let bytes: [u8; 4] = page[offset..offset + LEAF_NODE_KEY_SIZE]
            .try_into()
            .unwrap();
        u32::from_ne_bytes(bytes)
    }

    pub fn key(&self, cell_num: usize) -> u32 {
        Self::read_key(self.page, cell_num)
    }

    pub fn set_key(&mut self, cell_num: usize, key: u32) {
        let offset = Self::cell_offset(cell_num) + LEAF_NODE_KEY_OFFSET;
        self.page[offset..offset + LEAF_NODE_KEY_SIZE].copy_from_slice(&key.to_ne_bytes());
    }

    pub fn value_mut(&mut self, cell_num: usize) -> &mut [u8] {
        let offset = Self::cell_offset(cell_num) + LEAF_NODE_VALUE_OFFSET;
        &mut self.page[offset..offset + LEAF_NODE_VALUE_SIZE]
    }

    pub fn cell_slice(&self, cell_num: usize) -> &[u8] {
        let offset = Self::cell_offset(cell_num);
        &self.page[offset..offset + LEAF_NODE_CELL_SIZE]
    }

    pub fn cell_slice_mut(&mut self, cell_num: usize) -> &mut [u8] {
        let offset = Self::cell_offset(cell_num);
        &mut self.page[offset..offset + LEAF_NODE_CELL_SIZE]
    }

    pub fn find_cell(&self, key: u32) -> usize {
        let num_cells = self.num_cells() as usize;
        let mut min_index = 0;
        let mut one_past_max_index = num_cells;

        while one_past_max_index != min_index {
            let index = (min_index + one_past_max_index) / 2;
            let key_at_index = self.key(index);
            if key == key_at_index {
                return index;
            }
            if key < key_at_index {
                one_past_max_index = index;
            } else {
                min_index = index + 1;
            }
        }
        min_index
    }

    pub fn insert(&mut self, cell_num: usize, key: u32, value: &[u8]) {
        assert_eq!(value.len(), LEAF_NODE_VALUE_SIZE);
        let num_cells = self.num_cells() as usize;
        if num_cells >= LEAF_NODE_MAX_CELLS {
            panic!("Need to implement splitting a leaf node.");
        }

        if cell_num < num_cells {
            let src_start = Self::cell_offset(cell_num);
            let src_end = Self::cell_offset(num_cells);
            let dest_start = Self::cell_offset(cell_num + 1);
            self.page.copy_within(src_start..src_end, dest_start);
        }

        self.set_num_cells((num_cells + 1) as u32);
        self.set_key(cell_num, key);
        self.value_mut(cell_num).copy_from_slice(value);
    }

    pub fn print_tree(&self) {
        let num_cells = self.num_cells();
        println!("num_cells: {}", num_cells);
        for i in 0..num_cells as usize {
            println!(" - {} : {}", i, self.key(i));
        }
    }
}

pub struct InternalNode<'a> {
    pub page: &'a mut [u8],
}

impl<'a> InternalNode<'a> {
    pub fn new(page: &'a mut [u8]) -> Self {
        Self { page }
    }

    pub fn initialize(&mut self) {
        set_node_type(self.page, NodeType::Internal);
        set_node_root(self.page, false);
        self.set_num_keys(0);
        self.set_right_child(INVALID_PAGE_NUM);
    }

    pub fn update_key(&mut self, old_key: u32, new_key: u32) {
        let old_child_index = self.find_child(old_key);
        self.set_key(old_child_index, new_key);
    }

    pub fn cell_slice(&self, cell_num: usize) -> &[u8] {
        let offset = Self::cell_offset(cell_num);
        &self.page[offset..offset + INTERNAL_NODE_CELL_SIZE]
    }

    pub fn cell_slice_mut(&mut self, cell_num: usize) -> &mut [u8] {
        let offset = Self::cell_offset(cell_num);
        &mut self.page[offset..offset + INTERNAL_NODE_CELL_SIZE]
    }

    pub fn read_num_keys(page: &[u8]) -> u32 {
        let bytes: [u8; 4] = page[INTERNAL_NODE_NUM_KEYS_OFFSET
            ..INTERNAL_NODE_NUM_KEYS_OFFSET + INTERNAL_NODE_NUM_KEYS_SIZE]
            .try_into()
            .unwrap();
        u32::from_ne_bytes(bytes)
    }

    pub fn num_keys(&self) -> u32 {
        Self::read_num_keys(self.page)
    }

    pub fn set_num_keys(&mut self, count: u32) {
        self.page[INTERNAL_NODE_NUM_KEYS_OFFSET
            ..INTERNAL_NODE_NUM_KEYS_OFFSET + INTERNAL_NODE_NUM_KEYS_SIZE]
            .copy_from_slice(&count.to_ne_bytes());
    }

    pub fn right_child(&self) -> u32 {
        let bytes: [u8; 4] = self.page[INTERNAL_NODE_RIGHT_CHILD_OFFSET
            ..INTERNAL_NODE_RIGHT_CHILD_OFFSET + INTERNAL_NODE_RIGHT_CHILD_SIZE]
            .try_into()
            .unwrap();
        u32::from_ne_bytes(bytes)
    }

    pub fn set_right_child(&mut self, child_page_num: u32) {
        self.page[INTERNAL_NODE_RIGHT_CHILD_OFFSET
            ..INTERNAL_NODE_RIGHT_CHILD_OFFSET + INTERNAL_NODE_RIGHT_CHILD_SIZE]
            .copy_from_slice(&child_page_num.to_ne_bytes());
    }

    fn cell_offset(cell_num: usize) -> usize {
        INTERNAL_NODE_HEADER_SIZE + cell_num * INTERNAL_NODE_CELL_SIZE
    }

    pub fn child(&self, child_num: usize) -> u32 {
        let num_keys = self.num_keys() as usize;
        if child_num > num_keys {
            panic!(
                "Tried to access child_num {} > num_keys {}",
                child_num, num_keys
            );
        } else if child_num == num_keys {
            self.right_child()
        } else {
            let offset = Self::cell_offset(child_num);
            let bytes: [u8; 4] = self.page[offset..offset + INTERNAL_NODE_CHILD_SIZE]
                .try_into()
                .unwrap();
            u32::from_ne_bytes(bytes)
        }
    }

    pub fn set_child(&mut self, cell_num: usize, child_page_num: u32) {
        let num_keys = self.num_keys() as usize;
        if cell_num > num_keys {
            panic!(
                "Tried to set child_num {} > num_keys {}",
                cell_num, num_keys
            );
        } else if cell_num == num_keys {
            self.set_right_child(child_page_num);
        } else {
            let offset = Self::cell_offset(cell_num);
            self.page[offset..offset + INTERNAL_NODE_CHILD_SIZE]
                .copy_from_slice(&child_page_num.to_ne_bytes());
        }
    }

    pub fn read_key(page: &[u8], key_num: usize) -> u32 {
        let offset = Self::cell_offset(key_num) + INTERNAL_NODE_CHILD_SIZE;
        let bytes: [u8; 4] = page[offset..offset + INTERNAL_NODE_KEY_SIZE]
            .try_into()
            .unwrap();
        u32::from_ne_bytes(bytes)
    }

    pub fn key(&self, key_num: usize) -> u32 {
        Self::read_key(self.page, key_num)
    }

    pub fn set_key(&mut self, key_num: usize, key: u32) {
        let offset = Self::cell_offset(key_num) + INTERNAL_NODE_CHILD_SIZE;
        self.page[offset..offset + INTERNAL_NODE_KEY_SIZE].copy_from_slice(&key.to_ne_bytes());
    }

    pub fn find_child(&self, key: u32) -> usize {
        let num_keys = self.num_keys() as usize;
        let mut min_index = 0;
        let mut max_index = num_keys;

        while min_index != max_index {
            let index = (min_index + max_index) / 2;
            let key_to_right = self.key(index);
            if key_to_right >= key {
                max_index = index;
            } else {
                min_index = index + 1;
            }
        }
        min_index
    }
}
pub fn leaf_node_split_and_insert(
    pager: &mut Pager,
    root_page_num: usize,
    old_page_num: usize,
    cell_num: usize,
    key: u32,
    value: &[u8],
) {
    let new_page_num = pager.get_unused_page_num();
    pager.get_page(old_page_num);
    pager.get_page(new_page_num);

    // Record old_node's max key BEFORE redistributing cells
    let old_max = get_node_max_key(pager, old_page_num);

    let (old_page, new_page) = pager.get_two_pages_mut(old_page_num, new_page_num);
    let mut old_node = LeafNode::new(old_page.as_mut());
    let mut new_node = LeafNode::new(new_page.as_mut());
    new_node.initialize();

    // Link parent and next_leaf pointers
    set_node_parent(new_node.page, node_parent(old_node.page));
    new_node.set_next_leaf(old_node.next_leaf());
    old_node.set_next_leaf(new_page_num as u32);

    // Redistribute all 14 cells (13 existing + 1 new) between old_node (left) and new_node (right)
    for i in (0..=LEAF_NODE_MAX_CELLS).rev() {
        let index_within_node = i % LEAF_NODE_LEFT_SPLIT_COUNT;

        if i == cell_num {
            let destination_node = if i >= LEAF_NODE_LEFT_SPLIT_COUNT {
                &mut new_node
            } else {
                &mut old_node
            };
            destination_node.set_key(index_within_node, key);
            destination_node
                .value_mut(index_within_node)
                .copy_from_slice(value);
        } else {
            let src_idx = if i > cell_num { i - 1 } else { i };
            let mut cell_buf = [0u8; LEAF_NODE_CELL_SIZE];
            cell_buf.copy_from_slice(old_node.cell_slice(src_idx));

            let destination_node = if i >= LEAF_NODE_LEFT_SPLIT_COUNT {
                &mut new_node
            } else {
                &mut old_node
            };
            destination_node
                .cell_slice_mut(index_within_node)
                .copy_from_slice(&cell_buf);
        }
    }

    old_node.set_num_cells(LEAF_NODE_LEFT_SPLIT_COUNT as u32);
    new_node.set_num_cells(LEAF_NODE_RIGHT_SPLIT_COUNT as u32);

    if is_node_root(old_node.page) {
        create_new_root(pager, root_page_num, new_page_num);
    } else {
        let parent_page_num = node_parent(old_node.page) as usize;
        let new_max = get_node_max_key(pager, old_page_num);

        let was_right_child = {
            let parent_page = pager.get_page(parent_page_num);
            InternalNode::new(parent_page.as_mut()).right_child() as usize == old_page_num
        };

        if was_right_child {
            let new_right_max = get_node_max_key(pager, new_page_num);
            {
                let parent_page = pager.get_page(parent_page_num);
                InternalNode::new(parent_page.as_mut()).set_right_child(new_page_num as u32);
            }
            update_ancestor_max_keys(pager, parent_page_num, old_max, new_right_max);
            internal_node_insert(pager, root_page_num, parent_page_num, old_page_num);
        } else {
            {
                let parent_page = pager.get_page(parent_page_num);
                InternalNode::new(parent_page.as_mut()).update_key(old_max, new_max);
            }
            internal_node_insert(pager, root_page_num, parent_page_num, new_page_num);
        }
    }
}

pub fn internal_node_split_and_insert(
    pager: &mut Pager,
    root_page_num: usize,
    old_page_num: usize,
    child_page_num: usize,
) {
    let mut old_page_num = old_page_num;
    let old_max = get_node_max_key(pager, old_page_num);
    let child_max = get_node_max_key(pager, child_page_num);

    let new_page_num = pager.get_unused_page_num();
    {
        let new_page = pager.get_page(new_page_num);
        InternalNode::new(new_page.as_mut()).initialize();
    }

    let splitting_root = is_node_root(pager.get_page(old_page_num).as_ref());

    let parent_page_num = if splitting_root {
        create_new_root(pager, root_page_num, new_page_num);
        let root_page = pager.get_page(root_page_num);
        old_page_num = InternalNode::new(root_page.as_mut()).child(0) as usize;
        root_page_num
    } else {
        node_parent(pager.get_page(old_page_num).as_ref()) as usize
    };

    // 1. Collect all existing keys and children from `old_node`
    let (keys, children) = {
        let old_page = pager.get_page(old_page_num);
        let old_node = InternalNode::new(old_page.as_mut());
        let mut keys = Vec::with_capacity(INTERNAL_NODE_MAX_CELLS);
        let mut children = Vec::with_capacity(INTERNAL_NODE_MAX_CELLS + 1);
        for i in 0..INTERNAL_NODE_MAX_CELLS {
            keys.push(old_node.key(i));
            children.push(old_node.child(i) as usize);
        }
        children.push(old_node.right_child() as usize);
        (keys, children)
    };

    // 2. Left node (`old_node`) keeps the lower half
    let mid = INTERNAL_NODE_MAX_CELLS / 2;
    {
        let old_page = pager.get_page(old_page_num);
        let mut old_node = InternalNode::new(old_page.as_mut());
        old_node.set_num_keys(mid as u32);
        for i in 0..mid {
            old_node.set_child(i, children[i] as u32);
            old_node.set_key(i, keys[i]);
        }
        old_node.set_right_child(children[mid] as u32);
    }

    // 3. Right node (`new_node`) gets the upper half
    let right_num_keys = INTERNAL_NODE_MAX_CELLS - mid - 1;
    {
        let new_page = pager.get_page(new_page_num);
        let mut new_node = InternalNode::new(new_page.as_mut());
        new_node.set_num_keys(right_num_keys as u32);
        for (dst_idx, src_idx) in ((mid + 1)..INTERNAL_NODE_MAX_CELLS).enumerate() {
            new_node.set_child(dst_idx, children[src_idx] as u32);
            new_node.set_key(dst_idx, keys[src_idx]);
        }
        new_node.set_right_child(children[INTERNAL_NODE_MAX_CELLS] as u32);
    }

    // 4. Update parent pointers on all children that moved into `new_node`
    for &moved_child in &children[(mid + 1)..=INTERNAL_NODE_MAX_CELLS] {
        set_node_parent(pager.get_page(moved_child).as_mut(), new_page_num as u32);
    }

    // 5. Insert `child_page_num` into whichever half it belongs BEFORE computing `max_after_split`
    let old_split_max = get_node_max_key(pager, old_page_num);
    let destination_page_num = if child_max < old_split_max {
        old_page_num
    } else {
        new_page_num
    };

    internal_node_insert(pager, root_page_num, destination_page_num, child_page_num);
    set_node_parent(
        pager.get_page(child_page_num).as_mut(),
        destination_page_num as u32,
    );

    // Compute `max_after_split` AFTER `child_page_num` is inserted so the parent gets the true max key
    let max_after_split = get_node_max_key(pager, old_page_num);

    // 6 & 7. Update parent pointers/keys cleanly
    if splitting_root {
        let parent_page = pager.get_page(parent_page_num);
        InternalNode::new(parent_page.as_mut()).update_key(old_max, max_after_split);
    } else {
        let was_right_child = {
            let parent_page = pager.get_page(parent_page_num);
            InternalNode::new(parent_page.as_mut()).right_child() as usize == old_page_num
        };

        if was_right_child {
            {
                let parent_page = pager.get_page(parent_page_num);
                InternalNode::new(parent_page.as_mut()).set_right_child(new_page_num as u32);
            }
            set_node_parent(
                pager.get_page(new_page_num).as_mut(),
                parent_page_num as u32,
            );
            internal_node_insert(pager, root_page_num, parent_page_num, old_page_num);
        } else {
            {
                let parent_page = pager.get_page(parent_page_num);
                InternalNode::new(parent_page.as_mut()).update_key(old_max, max_after_split);
            }
            internal_node_insert(pager, root_page_num, parent_page_num, new_page_num);
            set_node_parent(
                pager.get_page(new_page_num).as_mut(),
                parent_page_num as u32,
            );
        }
    }
}

pub fn internal_node_insert(
    pager: &mut Pager,
    root_page_num: usize,
    parent_page_num: usize,
    child_page_num: usize,
) {
    let child_max_key = get_node_max_key(pager, child_page_num);

    let (index, original_num_keys, right_child_page_num) = {
        let parent_page = pager.get_page(parent_page_num);
        let parent = InternalNode::new(parent_page.as_mut());
        (
            parent.find_child(child_max_key),
            parent.num_keys() as usize,
            parent.right_child() as usize,
        )
    };

    if original_num_keys >= INTERNAL_NODE_MAX_CELLS {
        internal_node_split_and_insert(pager, root_page_num, parent_page_num, child_page_num);
        return;
    }

    // Handle inserting into an empty split internal node
    if right_child_page_num == INVALID_PAGE_NUM as usize {
        let parent_page = pager.get_page(parent_page_num);
        let mut parent = InternalNode::new(parent_page.as_mut());
        parent.set_right_child(child_page_num as u32);
        return;
    }

    let right_child_max_key = get_node_max_key(pager, right_child_page_num);

    let parent_page = pager.get_page(parent_page_num);
    let mut parent = InternalNode::new(parent_page.as_mut());
    parent.set_num_keys((original_num_keys + 1) as u32);

    if child_max_key > right_child_max_key {
        parent.set_child(original_num_keys, right_child_page_num as u32);
        parent.set_key(original_num_keys, right_child_max_key);
        parent.set_right_child(child_page_num as u32);
    } else {
        for i in (index + 1..=original_num_keys).rev() {
            let mut cell_buf = [0u8; INTERNAL_NODE_CELL_SIZE];
            cell_buf.copy_from_slice(parent.cell_slice(i - 1));
            parent.cell_slice_mut(i).copy_from_slice(&cell_buf);
        }
        parent.set_child(index, child_page_num as u32);
        parent.set_key(index, child_max_key);
    }
}

fn create_new_root(pager: &mut Pager, root_page_num: usize, right_child_page_num: usize) {
    let left_child_page_num = pager.get_unused_page_num();
    pager.get_page(root_page_num);
    pager.get_page(left_child_page_num);

    {
        let (root_page, left_child_page) =
            pager.get_two_pages_mut(root_page_num, left_child_page_num);
        left_child_page.copy_from_slice(root_page.as_ref());
        set_node_root(left_child_page.as_mut(), false);
    }

    let is_left_internal =
        get_node_type(pager.get_page(left_child_page_num).as_ref()) == NodeType::Internal;
    if is_left_internal {
        let (num_keys, right_child, children) = {
            let left_page = pager.get_page(left_child_page_num);
            let left_node = InternalNode::new(left_page.as_mut());
            let num_keys = left_node.num_keys() as usize;
            let mut children = Vec::with_capacity(num_keys);
            for i in 0..num_keys {
                children.push(left_node.child(i) as usize);
            }
            (num_keys, left_node.right_child() as usize, children)
        };

        for child_page_num in children {
            set_node_parent(
                pager.get_page(child_page_num).as_mut(),
                left_child_page_num as u32,
            );
        }
        if num_keys > 0 || right_child != INVALID_PAGE_NUM as usize {
            set_node_parent(
                pager.get_page(right_child).as_mut(),
                left_child_page_num as u32,
            );
        }
    }

    let left_child_max_key = get_node_max_key(pager, left_child_page_num);

    let root_page = pager.get_page(root_page_num);
    let mut root_node = InternalNode::new(root_page.as_mut());
    root_node.initialize();
    set_node_root(root_node.page, true);
    root_node.set_num_keys(1);
    root_node.set_child(0, left_child_page_num as u32);
    root_node.set_key(0, left_child_max_key);
    root_node.set_right_child(right_child_page_num as u32);

    set_node_parent(
        pager.get_page(left_child_page_num).as_mut(),
        root_page_num as u32,
    );
    set_node_parent(
        pager.get_page(right_child_page_num).as_mut(),
        root_page_num as u32,
    );
}

pub fn print_tree(pager: &mut Pager, page_num: usize, indentation_level: usize) {
    let indent = "  ".repeat(indentation_level);
    let page = pager.get_page(page_num);
    match get_node_type(page.as_ref()) {
        NodeType::Leaf => {
            let node = LeafNode::new(page.as_mut());
            let num_cells = node.num_cells();
            println!("{}leaf (size {})", indent, num_cells);
            for i in 0..num_cells as usize {
                println!("{}  - {}", indent, node.key(i));
            }
        }
        NodeType::Internal => {
            let node = InternalNode::new(page.as_mut());
            let num_keys = node.num_keys() as usize;
            let right_child = node.right_child() as usize;
            println!("{}internal (size {})", indent, num_keys);
            let mut entries = Vec::with_capacity(num_keys);
            for i in 0..num_keys {
                entries.push((node.child(i) as usize, node.key(i)));
            }
            for (child, key) in entries {
                print_tree(pager, child, indentation_level + 1);
                println!("{}  - key {}", indent, key);
            }
            print_tree(pager, right_child, indentation_level + 1);
        }
    }
}
pub fn print_constants() {
    println!("ROW_SIZE: {}", ROW_SIZE);
    println!("COMMON_NODE_HEADER_SIZE: {}", COMMON_NODE_HEADER_SIZE);
    println!("LEAF_NODE_HEADER_SIZE: {}", LEAF_NODE_HEADER_SIZE);
    println!("LEAF_NODE_CELL_SIZE: {}", LEAF_NODE_CELL_SIZE);
    println!("LEAF_NODE_SPACE_FOR_CELLS: {}", LEAF_NODE_SPACE_FOR_CELLS);
    println!("LEAF_NODE_MAX_CELLS: {}", LEAF_NODE_MAX_CELLS);
}
