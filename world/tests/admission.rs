#![recursion_limit = "1024"]
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
fn wide_enum_vector_charges_its_resident_layout_before_allocation() {
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

#[test]
fn returned_registration_error_rolls_back_dependencies_and_allows_retry() {
    thread_local! {
        static FAIL: std::cell::Cell<bool> = const { std::cell::Cell::new(true) };
        static CALLS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    }
    #[derive(Default, Component)]
    struct Dependency;
    #[derive(Default, Component)]
    struct Existing(u32);
    impl Resource for Existing {
        const NAME: &'static str = "Existing";
    }
    #[derive(Default, Data)]
    struct Hook;
    impl Component for Hook {
        const NAME: &'static str = "Hook";
        fn register(w: &mut World) -> Result<(), DataError> {
            CALLS.set(CALLS.get() + 1);
            w.register::<Hook>()?;
            w.register::<Dependency>()?;
            w.register_resource::<Existing>()?;
            if FAIL.get() {
                return Err(DataError::new("declined dependency"));
            }
            Ok(())
        }
    }
    let mut w = World::new(60, 0);
    w.register::<Existing>().unwrap();
    let entity = w.spawn(Existing(7)).unwrap();
    let consumer = w.subscribe_changes().unwrap();
    let before = w.save().unwrap();
    for _ in 0..3 {
        assert!(w.register::<Hook>().is_err());
        assert_eq!(w.save().unwrap(), before);
        assert!(w.spawn(Hook).is_err());
        assert!(w.spawn(Dependency).is_err());
        assert!(w.insert_resource(Existing(1)).is_err());
        assert_eq!(w.get::<Existing>(entity).unwrap().0, 7);
        assert_eq!(w.changes(&consumer).unwrap().events.len(), 0);
    }
    assert_eq!(CALLS.get(), 3);
    FAIL.set(false);
    w.register::<Hook>().unwrap();
    w.register::<Hook>().unwrap();
    assert_eq!(CALLS.get(), 4);
    w.spawn((Hook, Dependency)).unwrap();
    w.insert_resource(Existing(9)).unwrap();
    let saved = w.save().unwrap();
    w.load(&saved).unwrap();
    assert_eq!(w.save().unwrap(), saved);
}

#[test]
fn registration_chain_refuses_257_types_and_rolls_back_at_the_256_type_boundary() {
    thread_local! { static CALLS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) }; }
    macro_rules! chain {
        ($current:ident $(, $next:ident)*) => {
            #[derive(Default, Data)] struct $current;
            impl Component for $current {
                const NAME: &'static str = stringify!($current);
                fn register(w: &mut World) -> Result<(), DataError> {
                    CALLS.set(CALLS.get() + 1);
                    chain!(@next w $(, $next)*);
                    Ok(())
                }
            }
            chain!(@types $($next),*);
        };
        (@next $w:ident, $next:ident $(, $tail:ident)*) => { $w.register::<$next>()?; };
        (@next $w:ident) => { let _ = $w; };
        (@types $($next:ident),+) => { chain!($($next),+); };
        (@types) => {};
    }
    chain!(
        T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15, T16, T17, T18, T19,
        T20, T21, T22, T23, T24, T25, T26, T27, T28, T29, T30, T31, T32, T33, T34, T35, T36, T37,
        T38, T39, T40, T41, T42, T43, T44, T45, T46, T47, T48, T49, T50, T51, T52, T53, T54, T55,
        T56, T57, T58, T59, T60, T61, T62, T63, T64, T65, T66, T67, T68, T69, T70, T71, T72, T73,
        T74, T75, T76, T77, T78, T79, T80, T81, T82, T83, T84, T85, T86, T87, T88, T89, T90, T91,
        T92, T93, T94, T95, T96, T97, T98, T99, T100, T101, T102, T103, T104, T105, T106, T107,
        T108, T109, T110, T111, T112, T113, T114, T115, T116, T117, T118, T119, T120, T121, T122,
        T123, T124, T125, T126, T127, T128, T129, T130, T131, T132, T133, T134, T135, T136, T137,
        T138, T139, T140, T141, T142, T143, T144, T145, T146, T147, T148, T149, T150, T151, T152,
        T153, T154, T155, T156, T157, T158, T159, T160, T161, T162, T163, T164, T165, T166, T167,
        T168, T169, T170, T171, T172, T173, T174, T175, T176, T177, T178, T179, T180, T181, T182,
        T183, T184, T185, T186, T187, T188, T189, T190, T191, T192, T193, T194, T195, T196, T197,
        T198, T199, T200, T201, T202, T203, T204, T205, T206, T207, T208, T209, T210, T211, T212,
        T213, T214, T215, T216, T217, T218, T219, T220, T221, T222, T223, T224, T225, T226, T227,
        T228, T229, T230, T231, T232, T233, T234, T235, T236, T237, T238, T239, T240, T241, T242,
        T243, T244, T245, T246, T247, T248, T249, T250, T251, T252, T253, T254, T255, T256
    );
    let mut w = World::new(60, 0);
    let before = w.save().unwrap();
    assert!(w
        .register::<T0>()
        .err()
        .unwrap()
        .message
        .contains("type limit"));
    assert_eq!(CALLS.get(), 256);
    assert_eq!(w.save().unwrap(), before);
    w.register::<T1>().unwrap();
    assert_eq!(CALLS.get(), 512);
    w.register::<T1>().unwrap();
    assert_eq!(CALLS.get(), 512);
    assert!(w.register::<T0>().is_err());
}
