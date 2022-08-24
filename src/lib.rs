use std::ops::Index;

use mmap::{MapOption, MemoryMap};

struct EasyMap {
    map: MemoryMap,
    capacity: usize,
}

struct EasyMapIter<'a, T> {
    map: &'a EasyMap,
    index: usize,
    _v: T, // Hack to hold the type parameter
}

impl<'a> EasyMap {
    pub fn new<T: Sized>(capacity: usize) -> EasyMap {
        EasyMap {
            map: MemoryMap::new(
                capacity * std::mem::size_of::<T>(),
                &[MapOption::MapReadable, MapOption::MapWritable],
            )
            .unwrap(),
            capacity: capacity * std::mem::size_of::<T>(),
        }
    }

    fn put<T>(&mut self, offset: usize, value: T) {
        unsafe {
            self.map
                .data()
                .cast::<T>()
                .offset(offset as isize)
                .write(value);
        }
    }

    pub fn get<T>(&self, offset: usize) -> T {
        unsafe { self.map.data().cast::<T>().offset(offset as isize).read() }
    }

    pub fn len<T>(&self) -> usize {
        self.capacity / std::mem::size_of::<T>()
    }

    pub fn iter<T: Default>(&'a self) -> EasyMapIter<'a, T> {
        EasyMapIter::<T> {
            map: self,
            index: 0,
            _v: Default::default(),
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
        let map = EasyMap::new::<u32>(10);

        assert_eq!(map.len::<u32>(), 10);
        assert_eq!(map.len::<u64>(), 5);
    }

    #[test]
    fn map_write_read() {
        let map = &mut EasyMap::new::<u32>(1);
        map.put::<u32>(0, 1);

        assert_eq!(map.get::<u32>(0), 1);
    }

    #[test]
    fn map_iter() {
        let map = &mut EasyMap::new::<u32>(5);
        for i in 0..5 {
            map.put::<u32>(i, i as u32);
        }

        assert_eq!(map.iter::<u32>().collect::<Vec<_>>(), vec![0, 1, 2, 3, 4]);
    }
}
