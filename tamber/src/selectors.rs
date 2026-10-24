//! A reimport's search for the narrowest `when` a changed value needs
//! (`export::reimport_against`): the axis subsets it tries, bounded.

/// The narrowest `when` searched for at most this many candidates (Astra's
/// round 2 on the checker bound: singleton axes made the search exponential);
/// past it the line names every axis, which holds where the edit does.
pub(crate) const SELECTORS: usize = 4096;

/// Each `size`-subset of `0..n`, in order, made as it is asked for.
pub(crate) fn subsets(n: usize, size: usize) -> impl Iterator<Item = Vec<usize>> {
    let mut next = (size <= n).then(|| (0..size).collect::<Vec<usize>>());
    std::iter::from_fn(move || {
        let out = next.clone()?;
        let mut k = size;
        next = loop {
            if k == 0 {
                break None;
            }
            k -= 1;
            let pick = next.as_mut().expect("a pick");
            if pick[k] < n - size + k {
                pick[k] += 1;
                for j in k + 1..size {
                    pick[j] = pick[j - 1] + 1;
                }
                break next.take();
            }
        };
        Some(out)
    })
}

#[cfg(test)]
mod tests {
    use super::subsets;

    #[test]
    fn subsets_come_in_order_as_asked() {
        let all: Vec<Vec<usize>> = subsets(4, 2).collect();
        assert_eq!(
            all,
            vec![
                vec![0, 1],
                vec![0, 2],
                vec![0, 3],
                vec![1, 2],
                vec![1, 3],
                vec![2, 3]
            ]
        );
        assert_eq!(subsets(3, 0).collect::<Vec<_>>(), vec![Vec::<usize>::new()]);
        assert_eq!(subsets(2, 3).count(), 0);
        assert_eq!(subsets(3, 3).collect::<Vec<_>>(), vec![vec![0, 1, 2]]);
        // Lazily: the first of 2^40's worth without making the rest.
        assert_eq!(subsets(40, 20).next(), Some((0..20).collect::<Vec<_>>()));
    }
}
