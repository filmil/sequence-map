use zerocopy::AsBytes;
use zerocopy::FromBytes;

/// The possible types of an [Instance].
#[derive(Eq, PartialEq, Debug)]
pub enum Type {
    Empty = 0,
    StringPtr = 1,
    TablePtr = 2,
    Unknown = 255,
}

impl From<u8> for Type {
    fn from(t: u8) -> Type {
        if t == Type::Empty as u8 {
            return Type::Empty;
        }
        if t == Type::StringPtr as u8 {
            return Type::StringPtr;
        }
        if t == Type::TablePtr as u8 {
            return Type::TablePtr;
        }
        Type::Unknown
    }
}

#[derive(AsBytes, FromBytes)]
#[repr(packed)]
pub struct Instance {
    c_type: u8,
    /// Byte pointer index.  For strings, it's relative to the string offset
    /// index as specified in the table root.  For tables, it is relative to
    /// the start of the buffer.
    index: usize,
    /// For StringPtr, contains the actual key of the stored string.  Should be
    /// zero for all other types.
    string_key: u64,
}

impl Instance {
    pub fn string_index_and_key(&self) -> (usize, u64) {
        assert!(self.get_type() == Type::StringPtr);
        (self.index, self.string_key)
    }

    pub fn table_index(&self) -> usize {
        assert!(self.get_type() == Type::TablePtr);
        self.index
    }

    pub fn get_type(&self) -> Type {
        let t = self.c_type;
        if t == Type::Empty as u8 {
            return Type::Empty;
        }
        if t == Type::StringPtr as u8 {
            return Type::StringPtr;
        }
        if t == Type::TablePtr as u8 {
            return Type::TablePtr;
        }
        return Type::Unknown;
    }

    pub fn become_string_ptr(&mut self, index: usize, key: u64) {
        self.become_type(Type::StringPtr, index);
        self.string_key = key;
    }

    pub fn become_table_ptr(&mut self, index: usize) {
        self.become_type(Type::TablePtr, index);
    }

    fn become_type(&mut self, t: Type, index: usize) {
        self.c_type = t as u8;
        self.index = index;
        self.string_key = 0;
    }
}
