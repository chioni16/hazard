use std::{
    collections::HashMap,
    ops::{Deref, DerefMut},
};

#[derive(Debug, Clone)]
struct Map<K, V> {
    inner: HashMap<K, V>,
}

impl<K, V> Drop for Map<K, V> {
    fn drop(&mut self) {
        println!("drop called for map")
    }
}

impl<K, V> Deref for Map<K, V> {
    type Target = HashMap<K, V>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<K, V> DerefMut for Map<K, V> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}
