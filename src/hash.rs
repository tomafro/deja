use std::{
    collections::HashMap, error::Error, ffi::OsString, os::unix::ffi::OsStrExt, path::PathBuf,
};

use merkle_hash::{Algorithm, MerkleTree};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hash {
    hash: Vec<u8>,
}

impl Hash {
    pub fn hex(&self) -> String {
        merkle_hash::bytes_to_hex(&self.hash)
    }
}

impl std::fmt::Display for Hash {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", merkle_hash::bytes_to_hex(&self.hash))
    }
}

impl From<&[u8]> for Hash {
    fn from(bytes: &[u8]) -> Self {
        Hash {
            hash: Algorithm::Blake3.compute_hash(bytes),
        }
    }
}

impl From<&str> for Hash {
    fn from(s: &str) -> Self {
        Hash {
            hash: Algorithm::Blake3.compute_hash(s.as_bytes()),
        }
    }
}

impl From<&String> for Hash {
    fn from(s: &String) -> Self {
        Hash::from(s.as_str())
    }
}

impl From<&Option<String>> for Hash {
    fn from(s: &Option<String>) -> Self {
        if let Some(s) = s {
            Hash::from(s.as_str())
        } else {
            Hash::from(&[] as &[u8])
        }
    }
}

impl From<&std::option::Option<OsString>> for Hash {
    fn from(s: &std::option::Option<OsString>) -> Self {
        if let Some(s) = s {
            Hash::from(s.as_bytes())
        } else {
            Hash::from(&[] as &[u8])
        }
    }
}

impl TryFrom<&PathBuf> for Hash {
    type Error = anyhow::Error;

    fn try_from(path: &PathBuf) -> anyhow::Result<Self> {
        Ok(Hash {
            hash: MerkleTree::builder(path.to_str().unwrap())
                .hash_names(true)
                .build()
                .map_err(|e| {
                    println!("A {:?}", e);
                    if let Some(e) = e.source() {
                        println!("Error: {:?}", e);
                    }
                    e
                })?
                .root
                .item
                .hash,
        })
    }
}

impl From<&[Hash]> for Hash {
    fn from(hashes: &[Hash]) -> Self {
        let slices = hashes
            .iter()
            .map(|h| h.hash.as_slice())
            .collect::<Vec<&[u8]>>();

        Hash {
            hash: Algorithm::Blake3
                .compute_merkle_hash(slices.as_slice())
                .unwrap_or(Algorithm::Blake3.compute_hash(b"")),
        }
    }
}

impl From<&Vec<Hash>> for Hash {
    fn from(hashes: &Vec<Hash>) -> Self {
        let slices = hashes
            .iter()
            .map(|h| h.hash.as_slice())
            .collect::<Vec<&[u8]>>();

        Hash {
            hash: Algorithm::Blake3
                .compute_merkle_hash(slices.as_slice())
                .unwrap_or(Algorithm::Blake3.compute_hash(b"")),
        }
    }
}

impl TryFrom<&Vec<PathBuf>> for Hash {
    type Error = anyhow::Error;

    fn try_from(paths: &Vec<PathBuf>) -> anyhow::Result<Self> {
        let hashes = paths
            .iter()
            .map(Hash::try_from)
            .collect::<Result<Vec<Hash>, anyhow::Error>>();

        Ok(Hash::from(&hashes?))
    }
}

impl From<&Vec<String>> for Hash {
    fn from(strings: &Vec<String>) -> Self {
        let hashes = strings.iter().map(Hash::from).collect::<Vec<Hash>>();
        Hash::from(&hashes)
    }
}

impl From<&Vec<&str>> for Hash {
    fn from(strings: &Vec<&str>) -> Self {
        let hashes = strings
            .iter()
            .map(|p| Hash::from(p.as_bytes()))
            .collect::<Vec<Hash>>();
        Hash::from(&hashes)
    }
}

impl From<&HashMap<String, String>> for Hash {
    fn from(map: &HashMap<String, String>) -> Self {
        let mut entries = map.iter().collect::<Vec<(&String, &String)>>();
        entries.sort();
        let hashes = entries
            .iter()
            .map(|(k, v)| Hash::from(&vec![Hash::from(k.as_bytes()), Hash::from(v.as_bytes())]))
            .collect::<Vec<Hash>>();
        Hash::from(&hashes)
    }
}

#[cfg(test)]
mod test {
    use std::path::Path;

    use super::*;

    #[test]
    fn test_from_string() {
        assert_eq!(
            "ea8f163db38682925e4491c5e58d4bb3506ef8c14eb78a86e908c5624a67200f",
            Hash::from("hello").hex()
        );

        assert_eq!(
            "ea8f163db38682925e4491c5e58d4bb3506ef8c14eb78a86e908c5624a67200f",
            Hash::from(&"hello".to_string()).hex()
        );

        assert_eq!(
            "f94a694227c5f31a07551908ad5fb252f5f0964030df5f2f200adedfae4d9b69",
            Hash::from("goodbye").hex()
        );
    }

    #[test]
    fn test_from_strings() {
        assert_eq!(
            "ea8f163db38682925e4491c5e58d4bb3506ef8c14eb78a86e908c5624a67200f",
            Hash::from(&vec!["hello"]).hex()
        );

        assert_eq!(
            "ea8f163db38682925e4491c5e58d4bb3506ef8c14eb78a86e908c5624a67200f",
            Hash::from(&"hello".to_string()).hex()
        );

        assert_eq!(
            "f94a694227c5f31a07551908ad5fb252f5f0964030df5f2f200adedfae4d9b69",
            Hash::from("goodbye").hex()
        );
    }

    #[test]
    fn test_try_from_path() {
        assert_eq!(
            "a68f00ba89c19bbbfef24d6fe1e3dc7ca11758b1faba5d281c6865e96c45fd3d",
            Hash::try_from(&Path::new("test/fixtures/empty-a.txt").to_path_buf())
                .unwrap()
                .hex()
        );

        assert_eq!(
            "1cef27a2b5ed833e052e5e171757f4d4fe7d24354f5dfa594dfc17a16645bf4b",
            Hash::try_from(&Path::new("test/fixtures/empty-b.txt").to_path_buf())
                .unwrap()
                .hex()
        );
    }
}
