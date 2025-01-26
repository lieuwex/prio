use std::collections::{HashSet, VecDeque};

use rand::prelude::*;

use crate::File;

/*
pub fn take_n_random<T>(items: &mut VecDeque<T>, n: usize) -> Vec<T> {
    println!("take_n_random");
    let mut rng = thread_rng();
    let mut res = Vec::with_capacity(n);

    for _ in 0..n {
        if items.len() == 0 {
            panic!("vec is empty, but more items are requested");
        }

        let i = rng.gen_range(0..items.len());
        res.push(items.remove(i).unwrap());
    }

    res
}

pub fn take_n_most_interesting(items: &mut VecDeque<File>, n: usize) -> Vec<File> {
    println!("take_n_most_interesting");
    items
        .make_contiguous()
        .sort_by_key(|f| f.rating.deviation as i64);

    let mut res = Vec::with_capacity(n);
    for _ in 0..n {
        res.push(
            items
                .pop_back()
                .expect("vec is empty, but more items are requested"),
        );
    }
    res
}

pub fn take_n(items: &mut VecDeque<File>, n: usize) -> Vec<File> {
    let mut rng = thread_rng();

    let option = [(0, 70), (1, 30)]
        .choose_weighted(&mut rng, |i| i.1)
        .unwrap()
        .0;
    match option {
        0 => take_n_most_interesting(items, n),
        1 => take_n_random(items, n),
        _ => unreachable!(),
    }
}
*/

pub fn take_n(items: VecDeque<File>, n: usize) -> Vec<File> {
    let mut rng = thread_rng();

    // REVIEW: can we reduce collects?

    let items_ref: Vec<_> = items.iter().enumerate().collect();
    let indices: HashSet<usize> = items_ref
        .choose_multiple_weighted(&mut rng, n, |(_, f)| f.rating.deviation)
        .unwrap()
        .map(|i| i.0)
        .collect();

    items
        .into_iter()
        .enumerate()
        .filter(|(i, _)| indices.contains(i))
        .map(|(_, f)| f)
        .collect()
}
