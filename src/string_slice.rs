#![allow(dead_code)]

use std::collections::BTreeMap;
use std::ffi;
use std::string;

// Internally stores strings in a long sequence.  Same strings are deduped.
pub struct Intern {
    strings: Vec<u8>,
    seen: BTreeMap<string::String, usize>,
}

impl Into<Vec<u8>> for Intern {
    fn into(self) -> Vec<u8> {
        self.strings
    }
}

impl Intern {
    pub fn new() -> Intern {
        Intern {
            strings: vec![],
            seen: BTreeMap::new(),
        }
    }

    pub fn add(&mut self, s: &str) -> usize {
        let seen = self.seen.get(&s.to_string());
        match seen {
            Some(index) => {
                String::over(&self.strings[*index..]);
                *index
            }
            None => {
                let index = self.strings.len();
                let new_index = String::required_len(s) + index;
                self.seen.insert(s.to_string(), index);
                self.strings.resize(new_index, 0);
                String::init(s, &mut self.strings[index..new_index]);
                index
            }
        }
    }
    
    pub fn get(&self, index:usize) -> String<'_> {
        String::over(&self.strings[index..])
    }
}

#[allow(dead_code)]
#[derive(Debug, Eq, PartialEq)]
pub struct String<'a> {
    content: &'a ffi::CStr,
}

impl<'a> String<'a> {
    pub fn required_len(s: &str) -> usize {
        s.len() + 1
    }

    pub fn content(&self) -> &'a ffi::CStr {
        &self.content
    }

    pub fn to_str(&self) -> &'a str {
        &self.content.to_str().expect("to_str success")
    }

    pub fn to_string(&self) -> string::String {
        self.to_str().to_string()
    }

    // Initializes a string into the given buffer.  The buffer must have
    // enough space.
    pub fn init(src: &str, buffer: &'a mut [u8]) -> String<'a> {
        let required_len = String::required_len(src);
        assert!(
            required_len <= buffer.len(),
            "buffer len: {}, required_len: {}",
            buffer.len(),
            required_len
        );
        let str_bytes = src.as_bytes();
        buffer[..src.len()].clone_from_slice(str_bytes);
        buffer[str_bytes.len()] = 0;
        let content = std::ffi::CStr::from_bytes_with_nul(&buffer[..required_len])
            .expect("conversion is fine");
        String { content }
    }

    /// Overlays a string on top of the supplied buffer.
    pub fn over(buffer: &'a [u8]) -> String<'a> {
        let content =
            unsafe { std::ffi::CStr::from_ptr(buffer.as_ptr() as *const std::os::raw::c_char) };
        String { content }
    }
}

#[cfg(test)]
mod tests {
    use crate::string_slice::*;

    #[test]
    fn basic() {
        let mut intern = Intern::new();
        let sample_str = "Hello!";
        assert_eq!(sample_str.len() + 1, String::required_len(sample_str));

        let index = intern.add(sample_str);
        assert_eq!(index, 0);

        let c_string = intern.get(index).to_string();
        assert_eq!(c_string, sample_str);

        let index2 = intern.add("World!");
        let c_string_2 = intern.get(index2).to_string();
        assert_eq!(c_string_2, "World!");

        let expected: Vec<u8> = vec![72, 101, 108, 108, 111, 33, 0, 87, 111, 114, 108, 100, 33, 0];
        let actual: Vec<u8> = intern.into();
        assert_eq!(expected, actual);
    }

    fn deduplicate_seen_strings() {
        let mut intern = Intern::new();
        let index = intern.add("Hello!");
        let index2 = intern.add("World!");
        let index3 = intern.add("Hello!");
        assert_eq!(index, index3);
        assert_ne!(index, index2);

        assert_eq!(intern.get(index), intern.get(index3));
        assert_ne!(intern.get(index), intern.get(index2));
    }
}
