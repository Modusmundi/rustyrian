// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) rustyrian contributors

//! Serde support for fixed arrays of any length (serde's own impls stop at
//! 32), as flat sequences:
//!
//! |Field type|Attribute|
//! |---|---|
//! |`[T; N]`|`#[serde(with = "crate::serde_big")]`|
//! |`Box<[T; N]>`|`#[serde(with = "crate::serde_big::boxed")]`|
//! |`[[T; N]; M]`|`#[serde(with = "crate::serde_big::d2")]`|
//! |`Box<[[T; N]; M]>`|`#[serde(with = "crate::serde_big::d2_boxed")]`|

use serde::de::{Deserialize, Deserializer, Error};
use serde::ser::{Serialize, Serializer};

fn exact<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
    d: D,
    len: usize,
) -> Result<Vec<T>, D::Error> {
    let v = Vec::<T>::deserialize(d)?;
    if v.len() != len {
        return Err(D::Error::invalid_length(v.len(), &"a fixed-length array"));
    }
    Ok(v)
}

fn array<T, const N: usize>(it: &mut impl Iterator<Item = T>) -> [T; N] {
    std::array::from_fn(|_| it.next().expect("length checked by `exact`"))
}

pub fn serialize<S: Serializer, T: Serialize, const N: usize>(
    a: &[T; N],
    s: S,
) -> Result<S::Ok, S::Error> {
    s.collect_seq(a.iter())
}

pub fn deserialize<'de, D: Deserializer<'de>, T: Deserialize<'de>, const N: usize>(
    d: D,
) -> Result<[T; N], D::Error> {
    let mut it = exact::<D, T>(d, N)?.into_iter();
    Ok(array(&mut it))
}

pub mod boxed {
    use super::*;

    pub fn serialize<S: Serializer, T: Serialize, const N: usize>(
        a: &[T; N],
        s: S,
    ) -> Result<S::Ok, S::Error> {
        s.collect_seq(a.iter())
    }

    pub fn deserialize<'de, D: Deserializer<'de>, T: Deserialize<'de>, const N: usize>(
        d: D,
    ) -> Result<Box<[T; N]>, D::Error> {
        let v = exact::<D, T>(d, N)?;
        v.into_boxed_slice()
            .try_into()
            .map_err(|_| D::Error::custom("length mismatch"))
    }
}

pub mod d2 {
    use super::*;

    pub fn serialize<S: Serializer, T: Serialize, const N: usize, const M: usize>(
        a: &[[T; N]; M],
        s: S,
    ) -> Result<S::Ok, S::Error> {
        s.collect_seq(a.iter().flatten())
    }

    pub fn deserialize<
        'de,
        D: Deserializer<'de>,
        T: Deserialize<'de>,
        const N: usize,
        const M: usize,
    >(
        d: D,
    ) -> Result<[[T; N]; M], D::Error> {
        let mut it = exact::<D, T>(d, N * M)?.into_iter();
        Ok(std::array::from_fn(|_| array(&mut it)))
    }
}

pub mod d2_boxed {
    use super::*;

    pub fn serialize<S: Serializer, T: Serialize, const N: usize, const M: usize>(
        a: &[[T; N]; M],
        s: S,
    ) -> Result<S::Ok, S::Error> {
        s.collect_seq(a.iter().flatten())
    }

    pub fn deserialize<
        'de,
        D: Deserializer<'de>,
        T: Deserialize<'de>,
        const N: usize,
        const M: usize,
    >(
        d: D,
    ) -> Result<Box<[[T; N]; M]>, D::Error> {
        let mut it = exact::<D, T>(d, N * M)?.into_iter();
        let rows: Vec<[T; N]> = (0..M).map(|_| array(&mut it)).collect();
        rows.into_boxed_slice()
            .try_into()
            .map_err(|_| D::Error::custom("length mismatch"))
    }
}
