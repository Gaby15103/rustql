use crate::storage::{PAGE_SIZE, Pager, ROW_SIZE, Row};
use std::mem;

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

// Leaf Node Header Layout (10 bytes)
pub const LEAF_NODE_NUM_CELLS_SIZE: usize = std::mem::size_of::<u32>();
pub const LEAF_NODE_NUM_CELLS_OFFSET: usize = COMMON_NODE_HEADER_SIZE;
pub const LEAF_NODE_HEADER_SIZE: usize = COMMON_NODE_HEADER_SIZE + LEAF_NODE_NUM_CELLS_SIZE;

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

pub fn get_node_max_key(page: &[u8]) -> u32 {
    match get_node_type(page) {
        NodeType::Internal => {
            let num_keys = InternalNode::read_num_keys(page) as usize;
            InternalNode::read_key(page, num_keys - 1)
        }
        NodeType::Leaf => {
            let num_cells = LeafNode::read_num_cells(page) as usize;
            LeafNode::read_key(page, num_cells - 1)
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

    pub fn insert(&mut self, cell_num: usize, key: u32, value: &Row) {
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
        value.serialize(self.value_mut(cell_num));
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
        let offset = Self::cell_offset(cell_num);
        self.page[offset..offset + INTERNAL_NODE_CHILD_SIZE]
            .copy_from_slice(&child_page_num.to_ne_bytes());
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
}

pub fn leaf_node_split_and_insert(
    pager: &mut Pager,
    root_page_num: usize,
    old_page_num: usize,
    cell_num: usize,
    key: u32,
    value: &Row,
) {
    let new_page_num = pager.get_unused_page_num();
    pager.get_page(old_page_num);
    pager.get_page(new_page_num);
    let (old_page, new_page) = pager.get_two_pages_mut(old_page_num, new_page_num);
    let mut old_node = LeafNode::new(old_page.as_mut());
    let mut new_node = LeafNode::new(new_page.as_mut());
    new_node.initialize();
    for i in (0..=LEAF_NODE_MAX_CELLS).rev() {
        let index_within_node = i % LEAF_NODE_LEFT_SPLIT_COUNT;

        if i == cell_num {
            let destination_node = if i >= LEAF_NODE_LEFT_SPLIT_COUNT {
                &mut new_node
            } else {
                &mut old_node
            };
            destination_node.set_key(index_within_node, key);
            value.serialize(destination_node.value_mut(index_within_node));
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
        panic!("Need to implement updating parent after split");
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
    let left_child_max_key = {
        let left_child_page = pager.get_page(left_child_page_num);
        get_node_max_key(left_child_page.as_ref())
    };
    let root_page = pager.get_page(root_page_num);
    let mut root_node = InternalNode::new(root_page.as_mut());
    root_node.initialize();
    set_node_root(root_node.page, true);
    root_node.set_num_keys(1);
    root_node.set_child(0, left_child_page_num as u32);
    root_node.set_key(0, left_child_max_key);
    root_node.set_right_child(right_child_page_num as u32);
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
