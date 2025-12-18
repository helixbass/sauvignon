/// this is derived from `http`'s `Extensions` type
use std::any::{type_name, Any};
use std::collections::HashMap;
use std::fmt;

use smol_str::SmolStr;

type AnyMap = HashMap<SmolStr, Box<dyn AnyClone + Send + Sync>>;

#[derive(Clone, Default)]
pub struct AnyHashMap {
    map: Option<Box<AnyMap>>,
}

impl AnyHashMap {
    #[inline]
    pub fn new() -> AnyHashMap {
        AnyHashMap { map: None }
    }

    pub fn insert<T: Clone + Send + Sync + 'static>(&mut self, name: SmolStr, val: T) -> Option<T> {
        self.map
            .get_or_insert_with(Box::default)
            .insert(name, Box::new(val))
            .and_then(|boxed| boxed.into_any().downcast().ok().map(|boxed| *boxed))
    }

    pub fn get<T: Send + Sync + 'static>(&self, name: &str) -> Option<&T> {
        self.map
            .as_ref()
            .and_then(|map| map.get(name))
            .and_then(|boxed| (**boxed).as_any().downcast_ref())
    }

    pub fn get_mut<T: Send + Sync + 'static>(&mut self, name: &str) -> Option<&mut T> {
        self.map
            .as_mut()
            .and_then(|map| map.get_mut(name))
            .and_then(|boxed| (**boxed).as_any_mut().downcast_mut())
    }

    pub fn get_or_insert<T: Clone + Send + Sync + 'static>(
        &mut self,
        name: SmolStr,
        value: T,
    ) -> &mut T {
        self.get_or_insert_with(name, || value)
    }

    pub fn get_or_insert_with<T: Clone + Send + Sync + 'static, F: FnOnce() -> T>(
        &mut self,
        name: SmolStr,
        f: F,
    ) -> &mut T {
        let out = self
            .map
            .get_or_insert_with(Box::default)
            .entry(name)
            .or_insert_with(|| Box::new(f()));
        (**out).as_any_mut().downcast_mut().unwrap()
    }

    pub fn get_or_insert_default<T: Default + Clone + Send + Sync + 'static>(
        &mut self,
        name: SmolStr,
    ) -> &mut T {
        self.get_or_insert_with(name, T::default)
    }

    pub fn remove<T: Send + Sync + 'static>(&mut self, name: &str) -> Option<T> {
        self.map
            .as_mut()
            .and_then(|map| map.remove(name))
            .and_then(|boxed| boxed.into_any().downcast().ok().map(|boxed| *boxed))
    }

    pub fn clear(&mut self) {
        if let Some(ref mut map) = self.map {
            map.clear();
        }
    }

    pub fn is_empty(&self) -> bool {
        self.map.as_ref().map_or(true, |map| map.is_empty())
    }

    pub fn len(&self) -> usize {
        self.map.as_ref().map_or(0, |map| map.len())
    }

    pub fn extend(&mut self, other: Self) {
        if let Some(other) = other.map {
            if let Some(map) = &mut self.map {
                map.extend(*other);
            } else {
                self.map = Some(other);
            }
        }
    }

    pub fn contains_key(&self, name: &str) -> bool {
        self.map
            .as_ref()
            .map_or(false, |map| map.contains_key(name))
    }
}

impl fmt::Debug for AnyHashMap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        struct TypeName(&'static str);
        impl fmt::Debug for TypeName {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(self.0)
            }
        }

        let mut set = f.debug_set();
        if let Some(map) = &self.map {
            set.entries(
                map.iter()
                    .map(|(name, any_clone)| (name, TypeName(any_clone.as_ref().type_name()))),
            );
        }
        set.finish()
    }
}

trait AnyClone: Any {
    fn clone_box(&self) -> Box<dyn AnyClone + Send + Sync>;
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
    fn into_any(self: Box<Self>) -> Box<dyn Any>;
    fn type_name(&self) -> &'static str;
}

impl<T: Clone + Send + Sync + 'static> AnyClone for T {
    fn clone_box(&self) -> Box<dyn AnyClone + Send + Sync> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn into_any(self: Box<Self>) -> Box<dyn Any> {
        self
    }

    fn type_name(&self) -> &'static str {
        type_name::<T>()
    }
}

impl Clone for Box<dyn AnyClone + Send + Sync> {
    fn clone(&self) -> Self {
        (**self).clone_box()
    }
}

#[test]
fn test_any_hash_map() {
    #[derive(Clone, Debug, PartialEq)]
    struct MyType(i32);

    let mut any_hash_map = AnyHashMap::new();
    assert_eq!(format!("{any_hash_map:?}"), "{}");

    any_hash_map.insert("foo".to_owned(), 5i32);
    any_hash_map.insert("bar".to_owned(), MyType(10));

    assert_eq!(any_hash_map.get("foo"), Some(&5i32));
    assert_eq!(any_hash_map.get_mut("foo"), Some(&mut 5i32));

    let dbg = format!("{any_hash_map:?}");
    // map order is NOT deterministic
    assert!(
        (dbg == r#"{("bar", sauvignon::any_hash_map::test_any_hash_map::MyType), ("foo", i32)}"#)
            || (dbg
                == r#"{("foo", i32), ("bar", sauvignon::any_hash_map::test_any_hash_map::MyType)}"#),
        "{}",
        dbg
    );

    let any_hash_map_2 = any_hash_map.clone();

    assert_eq!(any_hash_map.remove::<i32>("foo"), Some(5i32));
    assert!(any_hash_map.get::<i32>("foo").is_none());

    // clone still has it
    assert_eq!(any_hash_map_2.get("foo"), Some(&5i32));
    assert_eq!(any_hash_map_2.get("bar"), Some(&MyType(10)));

    assert_eq!(any_hash_map.get::<bool>("bar"), None);
    assert_eq!(any_hash_map.get("bar"), Some(&MyType(10)));

    assert_eq!(any_hash_map_2.get::<i32>("not_foo"), None);
}
