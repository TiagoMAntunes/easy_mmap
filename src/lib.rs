use std::{fs, marker::PhantomData, os::unix::prelude::AsRawFd};

use mmap::{MapOption, MemoryMap};

pub struct EasyMap {
    map: MemoryMap,
    capacity: usize,
    _file: Option<fs::File>,
}

pub struct EasyMapIter<'a, T> {
    map: &'a EasyMap,
    index: usize,
    phantom: PhantomData<T>,
}

pub struct EasyMapBuilder<T> {
    file: Option<fs::File>,
    capacity: usize,
    options: Vec<MapOption>,
    _type: PhantomData<T>,
}

impl<'a> EasyMap {
    /// Creates a new EasyMap struct with enough capacity to hold AT LEAST `capacity` elements of type `T`.
    fn new(capacity: usize, options: &[MapOption], file: Option<fs::File>) -> EasyMap {
        EasyMap {
            map: MemoryMap::new(capacity, options).unwrap(),
            capacity,
            _file: file,
        }
    }

    /// Inserts a value at index `index` in the EasyMap according to the type of `T`.
    fn put<T>(&mut self, index: usize, value: T) {
        if index >= self.len::<T>() {
            panic!(
                "Index {} is out of bounds for type {}",
                index,
                std::any::type_name::<T>(),
            );
        }

        unsafe {
            self.map
                .data()
                .cast::<T>()
                .offset(index as isize)
                .write(value);
        }
    }

    /// Gets a value at index `index` in the EasyMap according to the type of `T`.
    pub fn get<T>(&self, index: usize) -> T {
        if index >= self.len::<T>() {
            panic!(
                "Index {} is out of bounds for type {}",
                index,
                std::any::type_name::<T>(),
            );
        }

        unsafe { self.map.data().cast::<T>().offset(index as isize).read() }
    }

    /// Returns how many elements of type `T` fit in the `EasyMap`.
    pub fn len<T>(&self) -> usize {
        self.capacity / std::mem::size_of::<T>()
    }

    /// Returns an iterator over `EasyMap` looking at the data as the type `T`.
    pub fn iter<T: Default>(&'a self) -> EasyMapIter<'a, T> {
        EasyMapIter::<T> {
            map: self,
            index: 0,
            phantom: PhantomData,
        }
    }
}

impl<T> Iterator for EasyMapIter<'_, T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index < self.map.len::<T>() {
            let item = self.map.get::<T>(self.index);
            self.index += 1;
            Some(item)
        } else {
            None
        }
    }
}

impl<T> EasyMapBuilder<T> {
    pub fn new() -> EasyMapBuilder<T> {
        EasyMapBuilder {
            file: None,
            capacity: 0,
            options: Vec::new(),
            _type: PhantomData,
        }
    }

    pub fn build(mut self) -> EasyMap {
        if self.file.is_some() {
            let file = self.file.unwrap();
            // allocate enough size in the file
            file.set_len((self.capacity * std::mem::size_of::<T>()) as u64)
                .unwrap();

            // Get file descriptor of file
            self.options.push(MapOption::MapFd(file.as_raw_fd()));

            self.file = Some(file);
        }

        EasyMap::new(
            self.capacity * std::mem::size_of::<T>(),
            &self.options,
            self.file,
        )
    }

    pub fn file(mut self, file: fs::File) -> EasyMapBuilder<T> {
        self.file = Some(file);
        self
    }

    pub fn capacity(mut self, capacity: usize) -> EasyMapBuilder<T> {
        self.capacity = capacity;
        self
    }

    pub fn options(mut self, options: &[MapOption]) -> EasyMapBuilder<T> {
        self.options = options.to_vec();
        self
    }

    pub fn add_option(mut self, option: MapOption) -> EasyMapBuilder<T> {
        self.options.push(option);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_create() {
        let map = &mut EasyMapBuilder::<u32>::new()
            .capacity(10)
            .options(&[])
            .build();

        assert_eq!(map.len::<u32>(), 10);
        assert_eq!(map.len::<u64>(), 5);
    }

    #[test]
    fn map_write_read() {
        let map = &mut EasyMapBuilder::<u32>::new()
            .capacity(1)
            .options(&[MapOption::MapReadable, MapOption::MapWritable])
            .build();
        map.put::<u32>(0, 1);

        assert_eq!(map.get::<u32>(0), 1);
    }

    #[test]
    fn map_iter() {
        let map = &mut EasyMapBuilder::<u32>::new()
            .capacity(5)
            .options(&[MapOption::MapReadable, MapOption::MapWritable])
            .build();
        for i in 0..5 {
            map.put::<u32>(i, i as u32);
        }

        assert_eq!(map.iter::<u32>().collect::<Vec<_>>(), vec![0, 1, 2, 3, 4]);
    }

    #[test]
    #[should_panic]
    fn map_oob_write() {
        let map = &mut EasyMapBuilder::<u32>::new()
            .capacity(1)
            .options(&[MapOption::MapReadable, MapOption::MapWritable])
            .build();

        map.put::<u32>(1, 1);
    }

    #[test]
    #[should_panic]
    fn map_oob_read() {
        let map = &mut EasyMapBuilder::<u32>::new()
            .capacity(1)
            .options(&[MapOption::MapReadable, MapOption::MapWritable])
            .build();

        map.get::<u32>(1);
    }

    #[test]
    fn map_create_file() {
        let file = fs::OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .open(format!("/tmp/map{}", rand::random::<u64>()))
            .unwrap();

        let map = &mut EasyMapBuilder::<u32>::new()
            .file(file)
            .capacity(10)
            .options(&[MapOption::MapReadable, MapOption::MapWritable])
            .build();

        assert_eq!(map.len::<u32>(), 10);

        // Check if file exists
        assert!(fs::metadata("/tmp/testmap").unwrap().is_file());

        // Write to file
        map.put::<u32>(0, 1);
        assert_eq!(map.get::<u32>(0), 1);
    }

    #[test]
    fn test_large_size() {
        let map = &mut EasyMapBuilder::<u64>::new()
            .capacity(65535)
            .options(&[MapOption::MapReadable, MapOption::MapWritable])
            .build();

        // Populate map
        for i in 0..65535 {
            map.put::<u64>(i, i as u64);
        }

        // Check if map is populated
        for i in 0..65535 {
            assert_eq!(map.get::<u64>(i), i as u64);
        }
    }

    #[test]
    fn test_struct() {
        struct TestStruct {
            v1: i64,
            v2: bool,
        }

        let length = 100000;

        let file = fs::OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .open(format!("/tmp/map{}", rand::random::<u64>()))
            .unwrap();

        let map = &mut EasyMapBuilder::<TestStruct>::new()
            .capacity(length)
            .options(&[MapOption::MapReadable, MapOption::MapWritable])
            .file(file)
            .build();

        for i in 0..length {
            map.put::<TestStruct>(
                i,
                TestStruct {
                    v1: i as i64,
                    v2: i % 2 == 0,
                },
            );
        }

        for i in 0..length {
            let s = map.get::<TestStruct>(i);
            assert_eq!(s.v1, i as i64);
            assert_eq!(s.v2, i % 2 == 0);
        }
    }
}
