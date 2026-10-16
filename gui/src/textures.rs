//! Cache LRU borné : garde en mémoire (GPU) les textures des vignettes récemment
//! affichées, et libère les plus anciennes au-delà d'une capacité. Sans borne, 5 000
//! vignettes de 400 px occuperaient ~2 Go.

use std::collections::HashMap;
use std::hash::Hash;

/// Cache LRU générique (les tests l'utilisent sans textures).
pub struct Lru<K, V> {
    entries: HashMap<K, (V, u64)>,
    clock: u64,
    capacity: usize,
}

impl<K: Eq + Hash + Clone, V> Lru<K, V> {
    /// Cache vide de `capacity` entrées au plus (au moins une).
    pub fn new(capacity: usize) -> Self {
        Lru {
            entries: HashMap::new(),
            clock: 0,
            capacity: capacity.max(1),
        }
    }

    /// Valeur de `key`, marquée comme récemment utilisée.
    pub fn get(&mut self, key: &K) -> Option<&V> {
        self.clock += 1;
        let clock = self.clock;
        self.entries.get_mut(key).map(|(v, used)| {
            *used = clock;
            &*v
        })
    }

    /// Vrai si `key` est présente (sans la marquer comme utilisée).
    #[cfg(test)]
    pub fn contains(&self, key: &K) -> bool {
        self.entries.contains_key(key)
    }

    /// Ajoute (ou remplace) une entrée ; supprime la moins récemment utilisée si la capacité
    /// est dépassée.
    pub fn insert(&mut self, key: K, value: V) {
        self.clock += 1;
        self.entries.insert(key, (value, self.clock));
        if self.entries.len() > self.capacity {
            let oldest = self
                .entries
                .iter()
                .min_by_key(|(_, (_, used))| *used)
                .map(|(k, _)| k.clone());
            if let Some(k) = oldest {
                self.entries.remove(&k);
            }
        }
    }

    /// Nombre d'entrées.
    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.entries.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evicts_least_recently_used() {
        let mut lru = Lru::new(2);
        lru.insert("a", 1);
        lru.insert("b", 2);
        assert_eq!(lru.get(&"a"), Some(&1)); // a devient récent
        lru.insert("c", 3); // b est le plus ancien
        assert!(lru.contains(&"a"));
        assert!(!lru.contains(&"b"));
        assert!(lru.contains(&"c"));
        assert_eq!(lru.len(), 2);
    }

    #[test]
    fn replacing_does_not_grow() {
        let mut lru = Lru::new(2);
        lru.insert("a", 1);
        lru.insert("a", 10);
        assert_eq!(lru.len(), 1);
        assert_eq!(lru.get(&"a"), Some(&10));
        assert_eq!(lru.get(&"z"), None);
    }

    #[test]
    fn capacity_is_at_least_one() {
        let mut lru = Lru::new(0);
        lru.insert(1, ());
        lru.insert(2, ());
        assert_eq!(lru.len(), 1);
        assert!(lru.contains(&2));
    }
}
