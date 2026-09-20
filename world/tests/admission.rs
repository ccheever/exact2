use exact_world::*;
#[allow(dead_code)]
#[path = "../src/storage/counting.rs"]
mod counting;

#[test]
fn finite_recursive_defaults_hash_and_encode_without_aborting() {
    const CHILD: &str = "EXACT_K1F_RECURSIVE_ADMISSION";
    if !cfg!(miri) && std::env::var_os(CHILD).is_none() {
        let result = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "finite_recursive_defaults_hash_and_encode_without_aborting",
            ])
            .env(CHILD, "1")
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        return;
    }
    type Link = Option<Box<Node>>;
    #[derive(Default, Data)]
    struct Node {
        #[data(skip)]
        next: Link,
    }
    let node = Node::default();
    assert_eq!(bin::to_vec(&node).unwrap(), [8, 0]);
    hash::of(&node).unwrap();
    assert!(bin::from_slice::<Node>(&[8, 0]).unwrap().next.is_none());
    #[derive(Default, Data)]
    struct Branch {
        next: Option<Box<Leaf>>,
    }
    #[derive(Default, Data)]
    struct Leaf {
        next: Option<Box<Branch>>,
    }
    let value = Branch {
        next: Some(Box::new(Leaf::default())),
    };
    let bytes = bin::to_vec(&value).unwrap();
    let loaded: Branch = bin::from_slice(&bytes).unwrap();
    assert!(loaded.next.unwrap().next.is_none());
    #[derive(Default, Data)]
    enum Tree {
        #[default]
        End,
        Branch(Box<Tree>),
    }
    let value = Tree::Branch(Box::new(Tree::End));
    let bytes = bin::to_vec(&value).unwrap();
    assert_eq!(
        hash::of(&value),
        hash::of(&bin::from_slice::<Tree>(&bytes).unwrap())
    );
}

#[test]
fn unit_default_enum_charges_its_largest_inline_variant_before_allocation() {
    #[allow(clippy::large_enum_variant)] // Adversarial inline layout is the regression.
    #[derive(Default, Component)]
    enum Wide {
        #[default]
        Empty,
        Full([[[u8; 32]; 32]; 32]),
    }
    let values: Vec<_> = (0..64).map(|_| Wide::Empty).collect();
    let bytes = bin::to_vec(&values).unwrap();
    let budget = data::LoadBudget::new(65_536);
    let (loaded, counts) =
        counting::measure(|| bin::from_slice_in::<Vec<Wide>>(&bytes, Some(&budget)));
    assert!(
        loaded.is_err(),
        "wide empty variants bypassed admission: {counts:?}"
    );
    assert!(counts.1 < 20_000, "allocation preceded refusal: {counts:?}");
    assert!(Wide::default_size() >= 32_768);
    let loaded = bin::from_slice::<Vec<Wide>>(&bytes).unwrap();
    assert_eq!(loaded.len(), 64);
    assert_eq!(bin::to_vec(&loaded).unwrap(), bytes);
}

#[test]
fn closed_map_keys_round_trip_unicode_and_distinct_names() {
    fn check<K: Data + Ord + From<&'static str> + std::fmt::Debug + PartialEq>()
    where
        std::collections::BTreeMap<K, u32>: Data,
    {
        let value = std::collections::BTreeMap::from([(K::from("x"), 7u32), (K::from("é🌕"), 9)]);
        let bytes = bin::to_vec(&value).unwrap();
        let loaded = bin::from_slice(&bytes).unwrap();
        assert_eq!(value, loaded);
        assert_eq!(bin::to_vec(&loaded).unwrap(), bytes);
        assert_eq!(hash::of(&value), hash::of(&loaded));
    }
    check::<String>();
    check::<std::rc::Rc<str>>();
}
