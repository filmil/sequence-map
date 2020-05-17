
use crate::cell;
use std::mem::size_of;
use zerocopy::AsBytes;
use zerocopy::FromBytes;
use zerocopy::LayoutVerified;

pub type TypeSize = u32;

#[derive(Debug, Eq, PartialEq)]
#[allow(dead_code)] // We want a zero value to be defined.
pub enum Type {
    // The table is empty.  This is not explicitly used, but is a consequence
    // of zero-initialization.
    Empty = 0,
    Root = 1,
    Table = 2,
    String = 3,
    Unknown = 255,
}

impl From<TypeSize> for Type {
    fn from(val: TypeSize) -> Type {
        if val == Type::Empty as TypeSize {
            return Type::Empty;
        }
        if val == Type::Root as TypeSize {
            return Type::Root;
        }
        if val == Type::Table as TypeSize {
            return Type::Table;
        }
        if val == Type::String as TypeSize {
            return Type::String;
        }
        Type::Unknown
    }
}

#[derive(Debug, AsBytes, FromBytes)]
#[repr(C)]
pub struct Root {
    pub htype: TypeSize,
    pad0: [u8; 4],
    pub root_table_offset: usize,
    pub string_offset: usize,
}

impl Root {
    pub fn set_type(&mut self, t: Type) {
        self.htype = t as TypeSize;
    }
    pub fn set_table_offset(&mut self, offset: usize) {
        self.root_table_offset = offset;
    }

    pub fn set_string_offset(&mut self, offset: usize) {
        self.string_offset = offset;
    }
}

#[derive(AsBytes, FromBytes)]
#[repr(C)]
pub struct TableHeader {
    pub htype: TypeSize,
    pad0: [u8; 4],
    // Number of bits in this table
    pub bits: u8,
    pad1: [u8; 7],
    // Followed by payload of 2^bits copies of cell::Instance.
}

impl TableHeader {
    pub fn set_bits(&mut self, bits: u8) {
        assert!(bits <= 8);
        self.htype = Type::Table as TypeSize;
        self.bits = bits;
    }
}

pub struct Table<'a> {
    pub header: &'a mut TableHeader,
    pub cells: &'a mut [cell::Instance],
}

impl<'a> Table<'a> {
    // Initializes a table for 2^bits entries.
    pub fn init(bits: u8, bytes: &'a mut [u8]) -> Table {
        assert!(bits <= 8);

        let (header, rest): (LayoutVerified<_, TableHeader>, _) =
            LayoutVerified::new_from_prefix_zeroed(bytes).unwrap();
        let elems = 1 << bits;
        let size = elems * size_of::<cell::Instance>();
        let cells = LayoutVerified::new_slice_zeroed(&mut rest[..size]).unwrap();
        let header = header.into_mut();
        header.set_bits(bits);
        let cells = cells.into_mut_slice();
        Table { header, cells }
    }

    // Overlays a table on top of this slice.  Assumes it is initialized.
    pub fn over(bytes: &'a mut [u8]) -> Table {
        let (header, rest): (LayoutVerified<_, TableHeader>, _) =
            LayoutVerified::new_from_prefix(bytes).unwrap();
        let header = header.into_mut();
        assert_eq!(Type::from(header.htype), Type::Table);
        let elems = 1 << header.bits;
        let size = elems * size_of::<cell::Instance>();
        let cells = LayoutVerified::new_slice(&mut rest[..size]).unwrap();
        let cells = cells.into_mut_slice();
        Table { header, cells }
    }

    pub fn cell_mut(&'a mut self, index: usize) -> &'a mut cell::Instance {
        &mut self.cells[index]
    }

    pub fn index(&self, key: u64) -> usize {
        let bits = self.header.bits;
        let bitmask: u64 = (1 << bits) - 1;
        let index = key & bitmask;
        index as usize
    }

    pub fn next_key(&self, key: u64) -> u64 {
        key >> self.header.bits
    }

    pub fn decrement_bits(&self, remaining: usize) -> usize {
        let bits_usize: usize = self.header.bits as usize;
        if remaining < bits_usize {
            return 0;
        }
        remaining - bits_usize
    }
}

#[derive(AsBytes, FromBytes)]
#[repr(C)]
pub struct String {
    htype: u32,
    pad0: [u8; 4],
    len: usize,
    // Followed by payload which ends with a '/0' byte.
}

#[derive(AsBytes, FromBytes)]
#[repr(C)]
pub struct Empty {
    htype: u32,
    pad0: [u8; 4],
}
