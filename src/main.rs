use std::{thread, sync::Arc};
use hazard::WRRMMap;

fn main() {
    let map = unsafe { WRRMMap::<usize, usize>::new() };


    let map = Arc::new(map);

    println!("{:#?}", map.get(&1));

    map.update(1, 2);
    map.update(3, 4);

    let map2 = Arc::clone(&map);
    let handle = thread::spawn(move || {
        map2.update(1, 7);
        map2.update(6, 12);
    });

    map.update(3, 8);
    map.update(5, 6);

    handle.join().unwrap();

    println!("1: {:?}", map.get(&1));
    println!("2: {:?}", map.get(&2));
    println!("3: {:?}", map.get(&3));
    println!("4: {:?}", map.get(&4));
    println!("5: {:?}", map.get(&5));
    println!("6: {:?}", map.get(&6));
    println!("7: {:?}", map.get(&7));
    println!("{:#?}", map);

}