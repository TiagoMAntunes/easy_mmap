use std::{fs, marker::PhantomData, os::unix::prelude::AsRawFd};

use mmap::{MapOption, MemoryMap};

/// This is the main struct of the library.
/// It owns a memory map and provides simplified access to this memory region
/// This memory region can support any type of data, as long as there is no dynamic memory regions within it (e.g. Box)
pub struct EasyMap<T> {
    map: MemoryMap,
    capacity: usize,
    _file: Option<fs::File>,
    _phantom: PhantomData<T>,
}

/// An iterator abstraction over the memory region.
/// It can be used to quickly iterate over the memory region
pub struct EasyMapIter<'a, T> {
    map: &'a EasyMap<T>,
    index: usize,
}

/// The builder class, that provides an easy interface to create the memory map with its respective requirements
pub struct EasyMapBuilder<T> {
    file: Option<fs::File>,
    capacity: usize,
    options: Vec<MapOption>,
    _type: PhantomData<T>,
}

impl<'a, T> EasyMap<T> {
    /// Creates a new EasyMap struct with enough capacity to hold AT LEAST `capacity` elements of type `T`.
    fn new(capacity: usize, options: &[MapOption], file: Option<fs::File>) -> EasyMap<T> {
        EasyMap {
            map: MemoryMap::new(capacity, options).unwrap(),
            capacity,
            _file: file,
            _phantom: PhantomData,
        }
    }

    /// Inserts a value at index `index` in the EasyMap according to the type of `T`.
    fn put(&mut self, index: usize, value: T) {
        if index >= self.len() {
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
    pub fn get(&self, index: usize) -> T {
        if index >= self.len() {
            panic!(
                "Index {} is out of bounds for type {}",
                index,
                std::any::type_name::<T>(),
            );
        }

        unsafe { self.map.data().cast::<T>().offset(index as isize).read() }
    }

    /// Returns how many elements of type `T` fit in the `EasyMap`.
    pub fn len(&self) -> usize {
        self.capacity / std::mem::size_of::<T>()
    }

    /// Returns an iterator over `EasyMap` looking at the data as the type `T`.
    pub fn iter(&'a self) -> EasyMapIter<'a, T> {
        EasyMapIter::<T> {
            map: self,
            index: 0,
        }
    }

    /// Due to the nature of a Mmap, memory cannot be consumed and will keep existing
    /// Typical workloads will not want to be creating new memory maps, and will prefer to update the memory in place
    /// We can then level an iterator over the memory region to simplify this process
    /// This function will consume the iterator and update the referenced memory region with the new values
    pub fn update_each(&mut self, f: impl Fn(usize, T) -> T) {
        for i in 0..self.len() {
            self.put(i, f(i, self.get(i)));
        }
    }
}

impl<T> Iterator for EasyMapIter<'_, T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index < self.map.len() {
            self.index += 1;
            Some(self.map.get(self.index - 1))
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

    /// Builds the memory map with the given requirements
    pub fn build(mut self) -> EasyMap<T> {
        if self.file.is_some() {
            let file = self.file.unwrap();
            // allocate enough size in the file
            file.set_len((self.capacity * std::mem::size_of::<T>()) as u64)
                .unwrap();

            // Get file descriptor of file
            self.options.push(MapOption::MapFd(file.as_raw_fd()));
            self.options // To make the code share the file in memory
                .push(MapOption::MapNonStandardFlags(libc::MAP_SHARED));

            self.file = Some(file);
        }

        EasyMap::new(
            self.capacity * std::mem::size_of::<T>(),
            &self.options,
            self.file,
        )
    }

    /// Passes the ownership of the file to the memory map
    pub fn file(mut self, file: fs::File) -> EasyMapBuilder<T> {
        self.file = Some(file);
        self
    }

    /// Sets the capacity that the file must have
    pub fn capacity(mut self, capacity: usize) -> EasyMapBuilder<T> {
        self.capacity = capacity;
        self
    }

    /// Batch sets the options that the file must have
    pub fn options(mut self, options: &[MapOption]) -> EasyMapBuilder<T> {
        self.options = options.to_vec();
        self
    }

    /// Adds an option to the memory region
    pub fn add_option(mut self, option: MapOption) -> EasyMapBuilder<T> {
        self.options.push(option);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_random_file() -> fs::File {
        fs::OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .open(format!("/tmp/map{}", rand::random::<u64>()))
            .unwrap()
    }

    #[test]
    fn map_create() {
        let map = &mut EasyMapBuilder::<u32>::new()
            .capacity(10)
            .options(&[])
            .build();

        assert_eq!(map.len(), 10);
    }

    #[test]
    fn map_write_read() {
        let map = &mut EasyMapBuilder::<u32>::new()
            .capacity(1)
            .options(&[MapOption::MapReadable, MapOption::MapWritable])
            .build();
        map.put(0, 1);

        assert_eq!(map.get(0), 1);
    }

    #[test]
    fn map_iter() {
        let map = &mut EasyMapBuilder::<u32>::new()
            .capacity(5)
            .options(&[MapOption::MapReadable, MapOption::MapWritable])
            .build();

        for i in 0..5 {
            map.put(i, i as u32);
        }

        assert_eq!(map.iter().collect::<Vec<_>>(), vec![0, 1, 2, 3, 4]);
    }

    #[test]
    #[should_panic]
    fn map_oob_write() {
        let map = &mut EasyMapBuilder::<u32>::new()
            .capacity(1)
            .options(&[MapOption::MapReadable, MapOption::MapWritable])
            .build();

        map.put(1, 1);
    }

    #[test]
    #[should_panic]
    fn map_oob_read() {
        let map = &mut EasyMapBuilder::<u32>::new()
            .capacity(1)
            .options(&[MapOption::MapReadable, MapOption::MapWritable])
            .build();

        map.get(1);
    }

    #[test]
    fn map_create_file() {
        let file = create_random_file();

        let map = &mut EasyMapBuilder::<u32>::new()
            .file(file)
            .capacity(10)
            .options(&[MapOption::MapReadable, MapOption::MapWritable])
            .build();

        assert_eq!(map.len(), 10);

        // Check if file exists
        assert!(fs::metadata("/tmp/testmap").unwrap().is_file());

        // Write to file
        map.put(0, 1);
        assert_eq!(map.get(0), 1);
    }

    #[test]
    fn test_large_size() {
        let map = &mut EasyMapBuilder::new()
            .capacity(65535)
            .options(&[MapOption::MapReadable, MapOption::MapWritable])
            .build();

        // Populate map
        for i in 0..65535 {
            map.put(i, i as u64);
        }

        // Check if map is populated
        for i in 0..65535 {
            assert_eq!(map.get(i), i as u64);
        }
    }

    #[test]
    fn test_struct() {
        struct TestStruct {
            v1: i64,
            v2: bool,
        }

        let length = 100000;

        let file = create_random_file();

        let map = &mut EasyMapBuilder::new()
            .capacity(length)
            .options(&[MapOption::MapReadable, MapOption::MapWritable])
            .file(file)
            .build();

        for i in 0..length {
            map.put(
                i,
                TestStruct {
                    v1: i as i64,
                    v2: i % 2 == 0,
                },
            );
        }

        for i in 0..length {
            let s = map.get(i);
            assert_eq!(s.v1, i as i64);
            assert_eq!(s.v2, i % 2 == 0);
        }
    }

    #[test]
    fn test_iter_write() {
        let file = create_random_file();

        let map = &mut EasyMapBuilder::new()
            .capacity(10)
            .options(&[MapOption::MapReadable, MapOption::MapWritable])
            .file(file)
            .build();

        map.update_each(|idx, _| idx as u32);

        assert_eq!(
            map.iter().collect::<Vec<_>>(),
            vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9]
        );

        let sum = map.iter().sum::<u32>();
        assert_eq!(sum, 45);

        map.update_each(|_, v| v + 1);
        assert_eq!(
            map.iter().collect::<Vec<_>>(),
            vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10]
        );
    }

    #[test]
    fn test_write_to_file() {
        let file = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open("/tmp/testmap")
            .unwrap();

        let map = &mut EasyMapBuilder::new()
            .capacity(10)
            .options(&[MapOption::MapReadable, MapOption::MapWritable])
            .file(file)
            .build();

        map.update_each(|idx, _| idx as u32);

        assert_eq!(
            map.iter().collect::<Vec<_>>(),
            vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9]
        );

        drop(map);

        let file = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open("/tmp/testmap")
            .unwrap();

        let map = &mut EasyMapBuilder::<u32>::new()
            .file(file)
            .capacity(10)
            .options(&[MapOption::MapReadable, MapOption::MapWritable])
            .build();

        assert_eq!(
            map.iter().collect::<Vec<_>>(),
            vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9]
        );
    }
}
