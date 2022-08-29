use std::{
    fs,
    marker::PhantomData,
    ops::{Index, IndexMut},
    os::unix::prelude::AsRawFd,
    slice::{Iter, IterMut},
};

pub use mmap::MapOption;
use mmap::MemoryMap;
use rayon::prelude::*;

/// The main abstraction over the `mmap` crate.
/// Owns a memory map and provides simplified and safe access to this memory region.
/// Also provides some additional features such as iterators over the data.
pub struct EasyMmap<'a, T> {
    _map: MemoryMap,
    _data: &'a mut [T],
    capacity: usize,
    _file: Option<fs::File>,
}

impl<'a, T> EasyMmap<'a, T>
where
    T: Copy,
{
    /// Creates a new EasyMmap struct with enough capacity to hold `capacity` elements of type `T`.
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

    /// How many elements can be stored in the memory map.
    pub fn len(&self) -> usize {
        self.capacity
    }

    /// Returns a read-only iterator over the elements of the memory map.
    pub fn iter(&self) -> Iter<'_, T> {
        self._data.iter()
    }

    /// Returns a mutable iterator over the elements of the memory map.
    pub fn iter_mut(&mut self) -> IterMut<'_, T> {
        self._data.iter_mut()
    }

    /// Returns a parallel iterator over the elements of the memory map.
    pub fn par_iter(&self) -> impl ParallelIterator<Item = &T> where T: Send + Sync {
        self._data.par_iter()
    }

    /// Returns a mutable parallel iterator over the elements of the memory map.
    pub fn par_iter_mut(&mut self) -> impl ParallelIterator<Item = &mut T> where T : Send + Sync{
        self._data.par_iter_mut()
    }

    /// Returns a read-only slice of the memory map data.
    pub fn get_data_as_slice(&self) -> &[T] {
        self._data
    }

    /// Returns a mutable slice of the memory map data.
    pub fn get_data_as_slice_mut(&mut self) -> &mut [T] {
        self._data
    }

    /// Convenience method for filling the memory map with a custom function
    /// Example:
    /// ```
    /// let mut mmap = easy_mmap::EasyMmapBuilder::new()
    ///                            .readable()
    ///                            .writable()
    ///                            .capacity(5)
    ///                            .build();
    ///
    /// mmap.fill(|i| i as u32);
    /// assert_eq!(mmap.get_data_as_slice(), &[0, 1, 2, 3, 4]);
    /// ```
    pub fn fill(&mut self, f: impl Fn(usize) -> T) {
        for (i, v) in self._data.iter_mut().enumerate() {
            *v = f(i);
        }
    }
}

/// The structure can be indexed similarly to an array.
/// Example:
/// ```
/// let mut mmap = easy_mmap::EasyMmapBuilder::new()
///                     .options(&[
///                         mmap::MapOption::MapWritable,
///                         mmap::MapOption::MapReadable,
///                     ])
///                     .capacity(10)
///                     .build();
/// mmap[0] = 1;
/// println!("{}", mmap[0]);
/// ```
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

/// The structure can be indexed an array or slice.
/// See the `Index` trait for an example.
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

/// The builder class for the EasyMmap struct.
/// Provides an easy-to-use interface to create a new EasyMmap struct.
pub struct EasyMmapBuilder<T> {
    file: Option<fs::File>,
    capacity: usize,
    options: Vec<MapOption>,
    _type: PhantomData<T>,
}

impl<'a, T> EasyMmapBuilder<T> {
    /// Creates a new EasyMmapBuilder struct.
    pub fn new() -> EasyMmapBuilder<T> {
        EasyMmapBuilder {
            file: None,
            capacity: 0,
            options: Vec::new(),
            _type: PhantomData,
        }
    }

    /// Builds the memory map with the given specifications.
    /// If the file has been specified, its size will be set to the requirements of the map.
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
    pub fn file(mut self, file: fs::File) -> EasyMmapBuilder<T> {
        self.file = Some(file);
        self
    }

    /// Sets the capacity that the mapped region must have.
    /// This capacity must be the number of objects of type `T` that can be stored in the memory map.
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

    pub fn readable(mut self) -> EasyMmapBuilder<T> {
        self.options.push(MapOption::MapReadable);
        self
    }

    pub fn writable(mut self) -> EasyMmapBuilder<T> {
        self.options.push(MapOption::MapWritable);
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

    #[test]
    fn easier_builder() {
        let mut map = EasyMmapBuilder::<i32>::new()
            .capacity(1)
            .readable()
            .writable()
            .build();

        map[0] = 1;
        assert_eq!(map[0], 1);
    }

    #[test]
    fn fill_constant() {
        let mut map = EasyMmapBuilder::<i32>::new()
            .capacity(5)
            .readable()
            .writable()
            .build();

        map.fill(|_| 1);
        assert_eq!(map.get_data_as_slice(), vec![1, 1, 1, 1, 1]);
    }

    #[test]
    fn fill_large() {
        let mut map = EasyMmapBuilder::<i32>::new()
            .capacity(100000)
            .readable()
            .writable()
            .build();

        map.fill(|i| i as i32);
        assert_eq!(map.get_data_as_slice(), (0..100000).collect::<Vec<_>>());
    }

    #[test]
    fn open_written_file() {
        let values = vec![1, 2, 3, 4, 5, 10, 20, 50];

        // Write to random file
        let filename = format!("/tmp/file{}", rand::random::<i32>());
        fs::write(&filename, &values).expect("Failed to write values to file");

        let file = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(false)
            .open(&filename)
            .expect("Failed to open file");

        // Now read the contents into the mmap
        let map = EasyMmapBuilder::<u8>::new()
            .capacity(values.len())
            .writable()
            .readable()
            .file(file)
            .build();

        assert_eq!(map.get_data_as_slice(), values);
    }

    #[test]
    fn parallel_iterators() {
        let mut map = EasyMmapBuilder::<i32>::new()
            .capacity(5)
            .options(&[MapOption::MapReadable, MapOption::MapWritable])
            .build();

        map.fill(|i| i as i32);

        assert_eq!(
            map.par_iter().map(|x| *x).collect::<Vec<_>>(),
            (0..5).collect::<Vec<_>>()
        );
    }

    #[test]
    fn parallel_iterators_mut() {
        let mut map = EasyMmapBuilder::<i32>::new()
            .capacity(5)
            .options(&[MapOption::MapReadable, MapOption::MapWritable])
            .build();

        map.fill(|i| i as i32);

        map.par_iter_mut().for_each(|x| *x += 1);

        assert_eq!(
            map.par_iter().map(|x| *x).collect::<Vec<_>>(),
            (1..6).collect::<Vec<_>>()
        );
    }
}
