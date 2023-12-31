
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::hash::Hash;
use std::sync::atomic::{AtomicBool, AtomicPtr, AtomicUsize, Ordering};
use std::fmt::Debug;

const RETIRE_LIMIT: usize = 5;

thread_local!(static RETIRE_LIST: RefCell<Vec<*mut std::os::raw::c_void>> = RefCell::new(vec![]));

#[derive(Debug)]
pub struct WRRMMap<K, V> {
    inner: AtomicPtr<HashMap<K, V>>,
    hazard_list: HazardList<K, V>,
}

impl<K: Clone + PartialEq + Eq + Hash + Debug + Default, V: Clone + Debug> WRRMMap<K, V> {
    /// Supports only one HashMap at any time
    pub unsafe fn new() -> Self {
        let map = Box::leak(Box::new(HashMap::new()));

        Self {
            inner: AtomicPtr::new(map as _),
            hazard_list: HazardList::new(),
        }
    }

    pub fn update(&self, key: K, val: V) {
        let mut old_ptr;

        loop {
            old_ptr = self.inner.load(Ordering::SeqCst);
            let old = unsafe { old_ptr.as_ref().unwrap() };

            let mut new = (*old).clone();
            new.insert(key.clone(), val.clone());
            let new = Box::leak(Box::new(new));

            if self
                .inner
                .compare_exchange(old_ptr, new as _, Ordering::SeqCst, Ordering::Relaxed)
                .is_ok()
            {
                break;
            }

            unsafe {
                std::ptr::drop_in_place(new as _);
            }
        }

        self.retire(old_ptr);
    }

    pub fn get(&self, key: &K) -> Option<V> {
        let node = self.hazard_list.acquire();
        loop {
            let ptr = self.inner.load(Ordering::SeqCst);
            node.hazard.store(ptr, Ordering::SeqCst);

            if self.inner.load(Ordering::SeqCst) == ptr {
                break;
            }
        }

        let map = unsafe { self.inner.load(Ordering::SeqCst).as_ref().unwrap() };
        let result = map.get(key).cloned();

        self.hazard_list.release(node);

        result
    }

    fn retire(&self, old: *mut HashMap<K, V>) {
        RETIRE_LIST.with(|rl| {
            rl.borrow_mut().push(old as _);
            if rl.borrow().len() >= RETIRE_LIMIT {
                self.hazard_list.scan();
            }
        });
    }
}

// Hazard pointer record
#[derive(Debug)]
struct HazardNode<K, V> {
    hazard: AtomicPtr<HashMap<K, V>>,
    next: AtomicPtr<Self>,
    active: AtomicBool,
}

impl<K, V> HazardNode<K, V> {
    fn new() -> *mut Self {
        let node = HazardNode {
            hazard: AtomicPtr::new(std::ptr::null_mut()),
            next: AtomicPtr::new(std::ptr::null_mut()),
            active: AtomicBool::new(true),
        };
        Box::into_raw(Box::new(node))
    }
}

#[derive(Debug)]
struct HazardList<K, V> {
    head: AtomicPtr<HazardNode<K, V>>,
    length: AtomicUsize,
}

impl<K, V> HazardList<K, V> {
    fn new() -> Self {
        Self {
            head: AtomicPtr::new(std::ptr::null_mut()),
            length: AtomicUsize::new(0),
        }
    }

    // Acquires one hazard pointer
    fn acquire(&self) -> &mut HazardNode<K, V> {
        // Try to reuse a retired HP record
        let mut node = &self.head;
        loop {
            let ptr = node.load(Ordering::SeqCst);
            if ptr.is_null() {
                break;
            }

            let hazard = unsafe { ptr.as_mut().unwrap() };
            if !hazard.active.load(Ordering::SeqCst)
                && hazard
                    .active
                    .compare_exchange(false, true, Ordering::SeqCst, Ordering::Relaxed)
                    .is_ok()
            {
                return hazard;
            }

            node = &hazard.next;
        }

        // Increment the list length
        self.length.fetch_add(1, Ordering::SeqCst);

        // Allocate a new one
        let new = HazardNode::new();
        let new = unsafe { new.as_mut().unwrap() };

        // Push it to the front
        loop {
            let old = self.head.load(Ordering::SeqCst);
            new.next.store(old, Ordering::SeqCst);
            if self
                .head
                .compare_exchange(old, new, Ordering::SeqCst, Ordering::Relaxed)
                .is_ok()
            {
                break;
            }
        }

        new
    }

    // Releases a hazard pointer
    fn release(&self, ptr: &mut HazardNode<K, V>) {
        ptr.hazard.store(std::ptr::null_mut(), Ordering::SeqCst);
        ptr.active.store(false, Ordering::SeqCst);
    }

    fn scan(&self) {
        // Stage 1: Scan hazard pointers list collecting all non-null ptrs
        let mut hp = HashSet::new();
        let mut head = &self.head;
        loop {
            let head_ptr = head.load(Ordering::SeqCst);
            if head_ptr.is_null() {
                break;
            }

            let head_ptr = unsafe { head_ptr.as_mut().unwrap() };
            let hazard = head_ptr.hazard.load(Ordering::SeqCst);
            if !hazard.is_null() {
                hp.insert(hazard as *mut std::os::raw::c_void);
            }

            head = &head_ptr.next;
        }

        // Stage 2: sort the hazard pointers
        // hp.sort_by_key(|&h| h as usize);

        // Stage 3: Search for’em!
        RETIRE_LIST.with(|rl| {
            let len = rl.borrow().len();
            for i in 0..len {
                if !hp.contains(&rl.borrow()[i]) {
                    let ptr = rl.borrow_mut().remove(i) as *mut HashMap<K, V>;
                    unsafe {
                        std::ptr::drop_in_place(ptr);
                    }
                }
            }
        })
    }
}
