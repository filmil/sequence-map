//![allow(dead_code)]

use std::mem::size_of;
use zerocopy::LayoutVerified;

mod cell;

pub(crate) mod string_slice;

pub(crate) mod header;

#[derive(Debug)]
pub struct Builder {
    bits: u8,
    index: Vec<u8>,
    strings: Vec<u8>,
}

impl Builder {
    pub fn new(bits: u8) -> Builder {
        Builder {
            bits,
            index: vec![],
            strings: vec![],
        }
    }

    fn allocate_string(&mut self, s: &str) -> usize {
        let index = self.strings.len();
        // Check whether a string like this one was already pushed.
        let new_index = index + string_slice::String::required_len(s);
        self.strings.resize(new_index, 0);
        println!(
            "strings: {:?}, index: {}, new_index: {}",
            &self.strings, index, new_index
        );
        string_slice::String::init(s, &mut self.strings[index..new_index]);
        index
    }

    fn header_unchecked(&mut self) -> &mut header::Root {
        let position = &mut self.index[..];
        assert!(position.len() >= size_of::<header::Root>());
        let (root, _): (LayoutVerified<_, header::Root>, _) =
            LayoutVerified::new_from_prefix(position).expect("header_unchecked");
        root.into_mut()
    }

    #[allow(unused_mut)]
    fn header(&mut self) -> &mut header::Root {
        let mut root = self.header_unchecked();
        assert_eq!(root.htype, header::Type::Root as header::TypeSize);
        root
    }

    fn reserve_header(&mut self) {
        assert_eq!(self.index.len(), 0);
        self.index
            .resize(self.index.len() + size_of::<header::Root>(), 0);
        let root = self.header_unchecked();
        root.set_type(header::Type::Root);
        root.set_table_offset(0);
        root.set_string_offset(0);
        assert_ne!(self.index.len(), 0);
    }

    fn append_table(&mut self) -> usize {
        let index = self.index.len();
        let entries: usize = 1 << self.bits;
        let size = size_of::<header::TableHeader>() + entries * size_of::<cell::Instance>();
        self.index.resize(index + size, 0);
        {
            header::Table::init(self.bits, &mut self.index[index..index + size]);
        }
        index
    }

    pub fn insert(&mut self, key: u64, value: &str) {
        if self.index.len() < size_of::<header::Root>() {
            self.reserve_header();
            // At this point both string and root index are zeroes.
        }
        let root_table_initialized = {
            let root = self.header();
            root.root_table_offset != 0
        };
        println!("root_table_initialized: {}", root_table_initialized);

        if !root_table_initialized {
            let index = self.append_table();
            assert_ne!(index, 0);
            let root = self.header();
            root.root_table_offset = index;
            // Now it is initialized.
            println!("root_table: {:?}", root);
        }
        let mut remaining_bits = 64;
        let mut running_key = key;
        let mut table_index = {
            let header = self.header();
            header.root_table_offset
        };
        assert_ne!(table_index, 0, "table: {:?}", self.index);
        loop {
            if remaining_bits == 0 {
                break;
            }
            let mut table = header::Table::over(&mut self.index[table_index..]);
            let index = table.index(key);
            // If it is empty, allocate string and put it here.
            // If it is already allocated, allocate new table and move string around.
            // If it is a table pointer, decrement and descend into table.
            let cell = table.cell_mut(index);
            let cell_type = cell.get_type();
            #[allow(unused_variables)]
            let cell = (); // Release self.
            match cell_type {
                cell::Type::Empty => {
                    let str_index = self.allocate_string(value);
                    let mut table = header::Table::over(&mut self.index[table_index..]);
                    let cell = table.cell_mut(index);
                    cell.become_string_ptr(str_index, key);
                    remaining_bits = 0; // exit the loop.
                }
                cell::Type::StringPtr => {
                    // There's already a string here.
                    let (str_index, str_key) = {
                        let mut table = header::Table::over(&mut self.index[table_index..]);
                        let cell = table.cell_mut(index);
                        // This is the string that was already here.
                        cell.string_index_and_key()
                    };

                    let new_table_index = self.append_table();
                    {
                        let mut table = header::Table::over(&mut self.index[table_index..]);
                        let cell = table.cell_mut(index);
                        cell.become_table_ptr(new_table_index);
                    }
                    {
                        let mut new_table = header::Table::over(&mut self.index[new_table_index..]);
                        let new_cell_index = 0; // ???
                        let cell = new_table.cell_mut(new_cell_index);
                        cell.become_string_ptr(str_index, str_key);
                    }
                    let table = header::Table::over(&mut self.index[table_index..]);
                    running_key = table.next_key(running_key);
                    remaining_bits = table.decrement_bits(remaining_bits);
                    // Continue into next iteration.
                }
                cell::Type::TablePtr => {
                    // Continue into next iteration.
                    {
                        let mut table = header::Table::over(&mut self.index[table_index..]);
                        let cell = table.cell_mut(index);
                        table_index = cell.table_index();
                    }
                    let table = header::Table::over(&mut self.index[table_index..]);
                    let table = table;
                    running_key = table.next_key(running_key);
                    remaining_bits = table.decrement_bits(remaining_bits);
                }
                cell::Type::Unknown => panic!("unknown cell type"),
            }
        }
        // If the root table is not initialized.
        // Initialize the root table.  Zero it out, write its table.
        // Write the pointer to the root table.
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }

    #[test]
    fn one_lookup() {
        let mut builder = Builder::new(2);

        builder.insert(42, "Hello!");
        builder.insert(84, "World!");

        println!("builder: {:?}", builder);
        assert!(false, "TBD");
    }
}
