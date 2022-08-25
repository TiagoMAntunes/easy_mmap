# easy_mmap

_Note: This crate is still in early development!_

This library provides a simple to user interface to manipulate memory mapped memory by forcing the usage of Rust's strong typing system. It's a simple abstraction over the [`mmap`](https://crates.io/crates/mmap) crate.

It further abstracts the memory mapped region by also supporting iterators and easy local updates.

Example usage:

```rust
use easy_mmap::EasyMmapBuilder;
use mmap::MapOption;

fn main() {
    let map = &mut EasyMmapBuilder::<u32>::new()
        .capacity(10)
        .options(&[MapOption::MapReadable, MapOption::MapWritable])
        .build();

    map.iter_mut()
        .enumerate()
        .for_each(|(idx, x)| *x = idx as u32);

    map.iter().for_each(|v| {
        print!("{} ", v);
    });
}
```
