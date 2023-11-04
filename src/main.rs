use std::{thread, sync::Arc};
use hazard::WRRMMap;

fn main() {
    let map = unsafe { WRRMMap::<usize, usize>::new() };

    println!("hello");

    let map = Arc::new(map);

    println!("{:#?}", map.get(&1));
    println!("hello2");

    map.update(1, 2);
    map.update(3, 4);

    println!("hello3");
    let map2 = Arc::clone(&map);
    let handle = thread::spawn(move || {
        assert_eq!(map2.get(&1), Some(2));
        assert_eq!(map2.get(&3), Some(4));
        map2.update(1, 7);
        assert_eq!(map2.get(&3), Some(4));
    });

    map.update(3, 8);
    map.update(5, 6);

    handle.join().unwrap();

    println!("{:#?}", map);

}