use super::*;

#[test]
fn a_reserved_time_publishes_in_both_content_region_profiles() {
    for profile in [
        exact_kernel::region::RegionProfile::PinnedOffers,
        exact_kernel::region::RegionProfile::SplitFacts,
    ] {
        let mut k = fixture();
        k.apply(
            0,
            1,
            &[
                Op::SetProp {
                    id: 4,
                    prop: PropId::Text,
                    value: "2 min. ago".into(),
                },
                Op::SetProp {
                    id: 4,
                    prop: PropId::TextReservation,
                    value: "59 min, 59 sec ago".into(),
                },
            ],
        )
        .unwrap();
        let mut ordinary = k.rehydrate(Box::new(MonospaceMeasurer::default()));
        ordinary
            .compute_layout(1, Offer::definite(400., 300.))
            .unwrap();
        k.set_content_region_profile(Some(binding(&k)), profile)
            .unwrap();
        let publication = ready(&mut k, 400., 1);
        assert!(publication.current);
        assert_eq!(
            publication.current_frame(key(&k, 4)).unwrap(),
            ordinary.node(4).unwrap().frame
        );
        k.apply(
            0,
            2,
            &[Op::SetProp {
                id: 4,
                prop: PropId::Text,
                value: "3 min. ago".into(),
            }],
        )
        .unwrap();
        assert!(ready(&mut k, 400., 1).current);
    }
}
