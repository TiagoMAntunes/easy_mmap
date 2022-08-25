use std::{
    fs,
    marker::PhantomData,
    ops::{Index, IndexMut},
    os::unix::prelude::AsRawFd,
    slice::{Iter, IterMut},
};

use mmap::{MapOption, MemoryMap};

/// This is the main struct of the library.
/// It owns a memory map and provides simplified access to this memory region.
/// This memory region can support any type of data, as long as there is no dynamic memory regions within it (e.g. Box).
pub struct EasyMmap<'a, T> {
    _map: MemoryMap,
    _data: &'a mut [T],
    capacity: usize,
    _file: Option<fs::File>,
}

/// The builder class, that provides an easy interface to create the memory map with its respective requirements.
impl<'a, T> EasyMmap<'a, T>
where
    T: Copy,
{
    /// Creates a new EasyMmap struct with enough capacity to hold AT LEAST `capacity` elements of type `T`.
    fn new(capacity: usize, options: &[MapOption], file: Option<fs::File>) -> EasyMmap<'a, T> {
        let map = MemoryMap::new(capacity * std::mem::size_of::<T>(), options).unwrap();
        let slice = unsafe { std::slice::from_raw_parts_mut(map.data().cast::<T>(), capacity) };

        EasyMmap {
            _map: map,
            _data: slice,
            capacity,
            _file: file,
        }
    }

    /// Returns how many elements of type `T` fit in the `EasyMmap`.
    pub fn len(&self) -> usize {
        self.capacity
    }

    /// Returns an iterator over `EasyMmap` looking at the data as the type `T`.
    pub fn iter(&self) -> Iter<'_, T> {
        self._data.iter()
    }

    pub fn iter_mut(&mut self) -> IterMut<'_, T> {
        self._data.iter_mut()
    }

    pub fn get_data_as_slice(&self) -> &[T] {
        self._data
    }

    pub fn get_data_as_slice_mut(&mut self) -> &mut [T] {
        self._data
    }
}

impl<'a, T> Index<usize> for EasyMmap<'a, T>
where
    T: Copy,
{
    type Output = T;

    fn index(&self, index: usize) -> &Self::Output {
        if index >= self.len() {
            panic!(
                "Index {} is out of bounds for type {}",
                index,
                std::any::type_name::<T>(),
            );
        };
        &self._data[index]
    }
}

impl<'a, T> IndexMut<usize> for EasyMmap<'a, T>
where
    T: Copy,
{
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        if index >= self.len() {
            panic!(
                "Index {} is out of bounds for type {}",
                index,
                std::any::type_name::<T>(),
            )
        }
        &mut self._data[index]
    }
}

pub struct EasyMmapBuilder<T> {
    file: Option<fs::File>,
    capacity: usize,
    options: Vec<MapOption>,
    _type: PhantomData<T>,
}

impl<'a, T> EasyMmapBuilder<T> {
    pub fn new() -> EasyMmapBuilder<T> {
        EasyMmapBuilder {
            file: None,
            capacity: 0,
            options: Vec::new(),
            _type: PhantomData,
        }
    }

    /// Builds the memory map with the given requirements.
    pub fn build(mut self) -> EasyMmap<'a, T>
    where
        T: Copy,
    {
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

        EasyMmap::new(self.capacity, &self.options, self.file)
    }

    /// Passes the ownership of the file to the memory map.
    /// Also sets the file to have enough size.
    pub fn file(mut self, file: fs::File) -> EasyMmapBuilder<T> {
        self.file = Some(file);
        self
    }

    /// Sets the capacity that the mapped region must have.
    pub fn capacity(mut self, capacity: usize) -> EasyMmapBuilder<T> {
        self.capacity = capacity;
        self
    }

    /// Batch sets the options that the mapped region must have.
    pub fn options(mut self, options: &[MapOption]) -> EasyMmapBuilder<T> {
        self.options = options.to_vec();
        self
    }

    /// Adds an individual option.
    pub fn add_option(mut self, option: MapOption) -> EasyMmapBuilder<T> {
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
        let map = &mut EasyMmapBuilder::<u32>::new()
            .capacity(10)
            .options(&[])
            .build();

        assert_eq!(map.len(), 10);
    }

    #[test]
    fn map_write_read() {
        let map = &mut EasyMmapBuilder::<u32>::new()
            .capacity(1)
            .options(&[MapOption::MapReadable, MapOption::MapWritable])
            .build();

        map[0] = 1;

        assert_eq!(map[0], 1);
    }

    #[test]
    fn map_iter() {
        let map = &mut EasyMmapBuilder::<u32>::new()
            .capacity(5)
            .options(&[MapOption::MapReadable, MapOption::MapWritable])
            .build();

        for i in 0..5 {
            map[i] = i as u32;
        }

        assert_eq!(
            map.iter().map(|x| *x).collect::<Vec<_>>(),
            vec![0, 1, 2, 3, 4]
        );
    }

    #[test]
    #[should_panic]
    fn map_oob_write() {
        let map = &mut EasyMmapBuilder::<u32>::new()
            .capacity(1)
            .options(&[MapOption::MapReadable, MapOption::MapWritable])
            .build();

        map[1] = 1;
    }

    #[test]
    #[should_panic]
    fn map_oob_read() {
        let map = &mut EasyMmapBuilder::<u32>::new()
            .capacity(1)
            .options(&[MapOption::MapReadable, MapOption::MapWritable])
            .build();

        map[1];
    }

    #[test]
    fn map_create_file() {
        let file = create_random_file();

        let map = &mut EasyMmapBuilder::<u32>::new()
            .file(file)
            .capacity(10)
            .options(&[MapOption::MapReadable, MapOption::MapWritable])
            .build();

        assert_eq!(map.len(), 10);

        // Check if file exists
        assert!(fs::metadata("/tmp/testmap").unwrap().is_file());

        // Write to file
        map[0] = 1;
        assert_eq!(map[0], 1);
    }

    #[test]
    fn test_large_size() {
        let map = &mut EasyMmapBuilder::new()
            .capacity(65535)
            .options(&[MapOption::MapReadable, MapOption::MapWritable])
            .build();

        // Populate map
        for i in 0..65535 {
            map[i] = i as u64;
        }

        // Check if map is populated
        for i in 0..65535 {
            assert_eq!(map[i], i as u64);
        }
    }

    #[test]
    fn test_struct() {
        #[derive(Clone, Copy)]
        struct TestStruct {
            v1: i64,
            v2: bool,
        }

        let length = 100000;

        let file = create_random_file();

        let map = &mut EasyMmapBuilder::new()
            .capacity(length)
            .options(&[MapOption::MapReadable, MapOption::MapWritable])
            .file(file)
            .build();

        for i in 0..length {
            map[i] = TestStruct {
                v1: i as i64,
                v2: i % 2 == 0,
            };
        }

        for i in 0..length {
            let s = map[i];
            assert_eq!(s.v1, i as i64);
            assert_eq!(s.v2, i % 2 == 0);
        }
    }

    #[test]
    fn test_iter() {
        let mut map = EasyMmapBuilder::<i32>::new()
            .capacity(5)
            .options(&[MapOption::MapReadable, MapOption::MapWritable])
            .build();

        for i in 0..5 {
            map[i] = i as i32;
        }

        for (i, x) in map.iter().enumerate() {
            assert_eq!(i as i32, *x);
        }
    }

    #[test]
    fn test_iter_mut() {
        let mut map = EasyMmapBuilder::<i32>::new()
            .capacity(5)
            .options(&[MapOption::MapReadable, MapOption::MapWritable])
            .build();

        for (i, x) in map.iter_mut().enumerate() {
            *x = i as i32;
        }

        for (i, x) in map.iter().enumerate() {
            assert_eq!(i as i32, *x);
        }
    }

    #[test]
    fn test_complex_iterator() {
        let mut map = EasyMmapBuilder::<u32>::new()
            .capacity(5)
            .options(&[MapOption::MapReadable, MapOption::MapWritable])
            .build();

        map.iter_mut()
            .enumerate()
            .for_each(|(idx, x)| *x = idx as u32);

        let v = map
            .iter()
            .map(|x| *x * 3)
            .filter(|x| x % 2 == 0)
            .collect::<Vec<u32>>();

        map.iter_mut().zip(v).for_each(|(x, y)| *x = y);

        assert_eq!(
            map.iter().map(|x| *x).collect::<Vec<_>>(),
            vec![0, 6, 12, 3, 4]
        );
    }

    #[test]
    fn get_data_slice() {
        let mut map = EasyMmapBuilder::<u32>::new()
            .capacity(5)
            .options(&[MapOption::MapReadable, MapOption::MapWritable])
            .build();

        map.iter_mut()
            .enumerate()
            .for_each(|(idx, x)| *x = idx as u32);

        let slice = map.get_data_as_slice();

        assert_eq!(slice.len(), 5);
        assert_eq!(slice[0], map[0]);
        assert_eq!(slice[1], map[1]);
        assert_eq!(slice[2], map[2]);
        assert_eq!(slice[3], map[3]);
        assert_eq!(slice[4], map[4]);

        let slice = map.get_data_as_slice_mut();
        assert_eq!(slice.len(), 5);
        slice[0] = 10;

        assert_eq!(map[0], 10);
    }
}
