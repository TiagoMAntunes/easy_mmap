use std::marker::PhantomData;

use mmap::{MapOption, MemoryMap};

pub struct EasyMap {
    map: MemoryMap,
    capacity: usize,
}

pub struct EasyMapIter<'a, T> {
    map: &'a EasyMap,
    index: usize,
    phantom: PhantomData<T>,
}

impl<'a> EasyMap {
    /// Creates a new EasyMap struct with enough capacity to hold AT LEAST `capacity` elements of type `T`.
    pub fn new<T: Sized>(capacity: usize, options: &[MapOption]) -> EasyMap {
        EasyMap {
            map: MemoryMap::new(capacity * std::mem::size_of::<T>(), options).unwrap(),
            capacity: capacity * std::mem::size_of::<T>(),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_create() {
        let map = EasyMap::new::<u32>(10, &[]);

        assert_eq!(map.len::<u32>(), 10);
        assert_eq!(map.len::<u64>(), 5);
    }

    #[test]
    fn map_write_read() {
        let map = &mut EasyMap::new::<u32>(1, &[MapOption::MapReadable, MapOption::MapWritable]);
        map.put::<u32>(0, 1);

        assert_eq!(map.get::<u32>(0), 1);
    }

    #[test]
    fn map_iter() {
        let map = &mut EasyMap::new::<u32>(5, &[MapOption::MapReadable, MapOption::MapWritable]);
        for i in 0..5 {
            map.put::<u32>(i, i as u32);
        }

        assert_eq!(map.iter::<u32>().collect::<Vec<_>>(), vec![0, 1, 2, 3, 4]);
    }

    #[test]
    #[should_panic]
    fn map_oob_write() {
        let map = &mut EasyMap::new::<u32>(1, &[MapOption::MapReadable, MapOption::MapWritable]);

        map.put::<u32>(1, 1);
    }

    #[test]
    #[should_panic]
    fn map_oob_read() {
        let map = &mut EasyMap::new::<u32>(1, &[MapOption::MapReadable, MapOption::MapWritable]);

        map.get::<u32>(1);
    }
}
