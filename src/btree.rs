use crate::storage::{PAGE_SIZE, ROW_SIZE, Row};
use std::mem;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum NodeType {
    Internal = 0,
    Leaf = 1,
}

// Common Node Header Layout (6 bytes total)
pub const NODE_TYPE_SIZE: usize = std::mem::size_of::<u8>();
pub const NODE_TYPE_OFFSET: usize = 0;
pub const IS_ROOT_SIZE: usize = std::mem::size_of::<u8>();
pub const IS_ROOT_OFFSET: usize = NODE_TYPE_OFFSET + NODE_TYPE_SIZE;
pub const PARENT_POINTER_SIZE: usize = std::mem::size_of::<u32>();
pub const PARENT_POINTER_OFFSET: usize = IS_ROOT_OFFSET + IS_ROOT_SIZE;
pub const COMMON_NODE_HEADER_SIZE: usize = NODE_TYPE_SIZE + IS_ROOT_SIZE + PARENT_POINTER_SIZE;

// Leaf Node Header Layout (adds num_cells: 4 bytes -> 10 bytes total)
pub const LEAF_NODE_NUM_CELLS_SIZE: usize = std::mem::size_of::<u32>();
pub const LEAF_NODE_NUM_CELLS_OFFSET: usize = COMMON_NODE_HEADER_SIZE;
pub const LEAF_NODE_HEADER_SIZE: usize = COMMON_NODE_HEADER_SIZE + LEAF_NODE_NUM_CELLS_SIZE;

// Leaf Node Body Layout (Key + Value)
pub const LEAF_NODE_KEY_SIZE: usize = std::mem::size_of::<u32>();
pub const LEAF_NODE_KEY_OFFSET: usize = 0;
pub const LEAF_NODE_VALUE_SIZE: usize = ROW_SIZE;
pub const LEAF_NODE_VALUE_OFFSET: usize = LEAF_NODE_KEY_OFFSET + LEAF_NODE_KEY_SIZE;
pub const LEAF_NODE_CELL_SIZE: usize = LEAF_NODE_KEY_SIZE + LEAF_NODE_VALUE_SIZE;
pub const LEAF_NODE_SPACE_FOR_CELLS: usize = PAGE_SIZE - LEAF_NODE_HEADER_SIZE;
pub const LEAF_NODE_MAX_CELLS: usize = LEAF_NODE_SPACE_FOR_CELLS / LEAF_NODE_CELL_SIZE;

pub struct LeafNode<'a> {
    page: &'a mut [u8],
}

pub fn get_node_type(page: &[u8]) -> NodeType {
    match page[NODE_TYPE_OFFSET] {
        0 => NodeType::Internal,
        1 => NodeType::Leaf,
        other => panic!("Unknown node type {}", other),
    }
}

impl<'a> LeafNode<'a> {
    pub fn new(page: &'a mut [u8]) -> Self {
        Self { page }
    }

    pub fn initialize(&mut self) {
        self.set_node_type(NodeType::Leaf);
        self.set_num_cells(0);
    }
    pub fn set_node_type(&mut self, node_type: NodeType) {
        self.page[NODE_TYPE_OFFSET] = node_type as u8;
    }

    pub fn num_cells(&self) -> u32 {
        let bytes: [u8; 4] = self.page
            [LEAF_NODE_NUM_CELLS_OFFSET..LEAF_NODE_NUM_CELLS_OFFSET + LEAF_NODE_NUM_CELLS_SIZE]
            .try_into()
            .unwrap();
        u32::from_ne_bytes(bytes)
    }

    pub fn set_num_cells(&mut self, num: u32) {
        self.page
            [LEAF_NODE_NUM_CELLS_OFFSET..LEAF_NODE_NUM_CELLS_OFFSET + LEAF_NODE_NUM_CELLS_SIZE]
            .copy_from_slice(&num.to_ne_bytes());
    }
    fn cell_offset(&self, cell_num: usize) -> usize {
        LEAF_NODE_HEADER_SIZE + cell_num * LEAF_NODE_CELL_SIZE
    }

    pub fn key(&self, cell_num: usize) -> u32 {
        let offset = self.cell_offset(cell_num) + LEAF_NODE_KEY_OFFSET;
        let bytes: [u8; 4] = self.page[offset..offset + LEAF_NODE_KEY_SIZE]
            .try_into()
            .unwrap();
        u32::from_ne_bytes(bytes)
    }
    pub fn set_key(&mut self, cell_num: usize, key: u32) {
        let offset = self.cell_offset(cell_num) + LEAF_NODE_KEY_OFFSET;
        self.page[offset..offset + LEAF_NODE_KEY_SIZE].copy_from_slice(&key.to_ne_bytes());
    }
    pub fn value(&self, cell_num: usize) -> &[u8] {
        let offset = self.cell_offset(cell_num) + LEAF_NODE_VALUE_OFFSET;
        &self.page[offset..offset + LEAF_NODE_VALUE_SIZE]
    }
    pub fn value_mut(&mut self, cell_num: usize) -> &mut [u8] {
        let offset = self.cell_offset(cell_num) + LEAF_NODE_VALUE_OFFSET;
        &mut self.page[offset..offset + LEAF_NODE_VALUE_SIZE]
    }

    pub fn insert(&mut self, cell_num: usize, key: u32, value: &Row) {
        let num_cells = self.num_cells() as usize;
        if num_cells >= LEAF_NODE_MAX_CELLS {
            panic!("Need to implement splitting a leaf node.")
        }

        if cell_num < num_cells {
            let src_start = self.cell_offset(cell_num);
            let src_end = self.cell_offset(num_cells);
            let dest_start = self.cell_offset(cell_num + 1);
            self.page.copy_within(src_start..src_end, dest_start)
        }

        self.set_num_cells((num_cells + 1) as u32);
        self.set_key(cell_num, key);
        value.serialize(self.value_mut(cell_num));
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

    pub fn print_tree(&self) {
        let num_cells = self.num_cells();
        println!("num_cells: {}", num_cells);
        for i in 0..num_cells as usize {
            println!(" - {} : {}", i, self.key(i));
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
