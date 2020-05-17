use std::mem::size_of;
use zerocopy::LayoutVerified;

mod cell;

pub(crate) mod string_slice;

pub(crate) mod header;

#[derive(Debug)]
pub struct Builder {
    bits: u8,
    index: Vec<u8>,
    strings: string_slice::Intern,
}

impl Builder {
    /// Creates a new map builder.  `bits` determines how many bits are used
    /// for each level of the internal trie, min bits is 2, and max is 16
    pub fn new(bits: usize) -> Builder {
        assert!(bits >= 2 && bits <= 16);
        let mut builder = Builder {
            bits: bits as u8,
            index: vec![],
            strings: string_slice::Intern::new(),
        };
        builder.reserve_header();
        builder
    }

    fn allocate_string(&mut self, s: &str) -> usize {
        self.strings.add(s)
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
            header::TableMut::init(self.bits, &mut self.index[index..index + size]);
        }
        index
    }

    pub fn build(mut self) -> Vec<u8> {
        {
            let len = self.index.len();
            // This will fail if nothing has been inserted!
            let root = self.header();
            root.set_string_offset(len);
        }
        let mut result = self.index;
        let mut strings: Vec<u8> = self.strings.into();
        result.append(&mut strings);
        result
    }

    pub fn insert(&mut self, key: u64, value: &str) {
        let root_table_initialized = {
            let root = self.header();
            root.root_table_offset != 0
        };

        if !root_table_initialized {
            let index = self.append_table();
            assert_ne!(index, 0);
            let root = self.header();
            root.root_table_offset = index;
            // Now it is initialized.
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
            let mut table = header::TableMut::overlay_mut(&mut self.index[table_index..]);
            let index = table.index(running_key);
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
                    let mut table = header::TableMut::overlay_mut(&mut self.index[table_index..]);
                    let cell = table.cell_mut(index);
                    cell.become_string_ptr(str_index, key);
                    remaining_bits = 0; // exit the loop.
                }
                cell::Type::StringPtr => {
                    // There's already a string here.
                    let (str_index, str_key) = {
                        let mut table =
                            header::TableMut::overlay_mut(&mut self.index[table_index..]);
                        let cell = table.cell_mut(index);
                        // This is the string that was already here.
                        cell.string_index_and_key()
                    };

                    // Adjust the key of the string which was already there.
                    assert!(remaining_bits <= 64, "remaining_bits: {}", remaining_bits);
                    let new_str_key = str_key >> (64 - remaining_bits);

                    let new_table_index = self.append_table();
                    {
                        // Create a new table to place the old string into.
                        let mut table =
                            header::TableMut::overlay_mut(&mut self.index[table_index..]);
                        let cell = table.cell_mut(index);
                        cell.become_table_ptr(new_table_index);
                    }
                    {
                        let mut new_table =
                            header::TableMut::overlay_mut(&mut self.index[new_table_index..]);
                        let new_str_key = new_table.next_key(new_str_key);
                        let new_cell_index = new_table.index(new_str_key);
                        let cell = new_table.cell_mut(new_cell_index);
                        cell.become_string_ptr(str_index, str_key);
                    }
                    // Now that we created the table, repeat this iteration.
                }
                cell::Type::TablePtr => {
                    // Chase the table pointer.
                    {
                        let mut table =
                            header::TableMut::overlay_mut(&mut self.index[table_index..]);
                        let cell = table.cell_mut(index);
                        table_index = cell.table_index();
                    }
                    let table = header::TableMut::overlay_mut(&mut self.index[table_index..]);
                    running_key = table.next_key(running_key);
                    remaining_bits = table.decrement_bits(remaining_bits);
                }
                cell::Type::Unknown => panic!("unknown cell type"),
            }
        }
    }
}

pub struct Map<'a> {
    rep: &'a [u8],
}

impl<'a> Map<'a> {
    pub fn new(rep: &'a [u8]) -> Map<'a> {
        Map { rep }
    }

    pub fn get(&'a self, key: u64) -> Option<&'a str> {
        use std::ffi::CStr;
        use std::os::raw::c_char;

        let (table_index, string_offset) = {
            let header = self.header();
            (header.root_table_offset, header.string_offset)
        };
        assert!(table_index > 0);
        let mut remaining_bits = 64;
        let mut running_key = key;
        let mut running_table_index = table_index;
        loop {
            if remaining_bits == 0 {
                break;
            }
            let table = header::Table::overlay(&self.rep[running_table_index..]);
            let index = table.index(running_key);
            let cell = table.cell(index);
            let cell_type = cell.get_type();
            match cell_type {
                cell::Type::Empty => {
                    return None;
                }
                cell::Type::StringPtr => {
                    let (string_index, string_key) = cell.string_index_and_key();
                    match key == string_key {
                        false => return None,
                        true => {
                            // Find that string.
                            let string_index = string_offset + string_index;
                            let cstr = unsafe {
                                // We know that the strings in the intern table
                                // are C strings (UTF-8 with a trailing '/0').
                                let ptr = self.rep[string_index..].as_ptr() as *const c_char;
                                CStr::from_ptr(ptr)
                            };
                            return Some(cstr.to_str().expect("UTF8 worked"));
                        }
                    }
                }
                cell::Type::TablePtr => {
                    remaining_bits = table.decrement_bits(remaining_bits);
                    running_key = table.next_key(running_key);
                    running_table_index = cell.table_index();
                    // Descend one level deeper.
                }
                cell::Type::Unknown => {
                    panic!("reached unknown cell");
                }
            }
        }
        None
    }

    fn header(&'a self) -> &'a header::Root {
        assert!(self.rep.len() >= size_of::<header::Root>());
        let (root, _): (LayoutVerified<_, header::Root>, _) =
            LayoutVerified::new_from_prefix(&self.rep[..]).expect("header check");
        root.into_ref()
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn basic() {
        let mut builder = Builder::new(2);
        builder.insert(42, "Hello!");
        builder.insert(84, "World!");
        let expected: Vec<u8> = vec![
            1, 0, 0, 0, 0, 0, 0, 0, 24, 0, 0, 0, 0, 0, 0, 0, 108, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0,
            0, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 1, 7, 0, 0, 0, 0, 0, 0, 0, 84, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 42, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 72, 101, 108, 108,
            111, 33, 0, 87, 111, 114, 108, 100, 33, 0,
        ];
        assert_eq!(expected, builder.build());
    }

    #[test]
    fn no_insert() {
        let builder = Builder::new(2);
        builder.build();
    }

    #[test]
    fn get_one_string() {
        let mut builder = Builder::new(2);
        builder.insert(42, "Hello!");
        let bytes = builder.build();
        let lookup = Map::new(&bytes);
        assert_eq!("Hello!", lookup.get(42).unwrap());
    }

    #[test]
    fn get_two_strings() {
        let mut builder = Builder::new(7);
        builder.insert(0x11_11_11, "World!");
        builder.insert(0x22, "Again!!");
        builder.insert(0x11, "Yadda!");
        builder.insert(0x11_11, "Diddy!");
        let bytes = builder.build();
        // This should not need to be mutable!
        let lookup = Map::new(&bytes);
        assert_eq!("Yadda!", lookup.get(0x11).unwrap());
        assert_eq!("Diddy!", lookup.get(0x11_11).unwrap());
        assert_eq!("Again!!", lookup.get(0x22).unwrap());
        assert_eq!("World!", lookup.get(0x11_11_11).unwrap());
    }

    fn insert_and_lookup_random_strings(bits: usize) {
        let mut reference_map = BTreeMap::new();
        let mut builder = Builder::new(bits);
        for entry in 0..1000 {
            let entry_str = format!("entry_{}", entry);
            reference_map.insert(entry, entry_str.clone());
            builder.insert(entry, &entry_str);
        }

        let buffer = builder.build();
        let lookup = Map::new(&buffer);
        for (key, value) in &reference_map {
            assert_eq!(
                lookup.get(*key).unwrap(),
                *value,
                "while looking up: key={}, value={}, bits={}",
                key,
                value,
                bits
            );
        }
    }

    #[test]
    fn test_insert_and_lookup_for_bits() {
        for bits in 2..16 {
            insert_and_lookup_random_strings(bits);
        }
    }
}
