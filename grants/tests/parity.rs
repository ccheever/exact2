use exact_grants::{GrantSet, Operation, Origin};

#[test]
fn native_and_browser_use_the_same_grant_corpus() {
    let cases: serde_json::Value =
        serde_json::from_str(include_str!("../../host/web/tests/fixtures/grants.json")).unwrap();
    for case in cases.as_array().unwrap() {
        let spec = case["spec"].as_str().unwrap();
        let parsed = GrantSet::parse(spec);
        let native = ibex2::grant::GrantSet::parse(spec);
        assert_eq!(
            native.is_ok(),
            parsed.is_ok(),
            "Ibex grammar: {spec}: {native:?}"
        );
        assert_eq!(
            parsed.is_ok(),
            case["ok"].as_bool().unwrap(),
            "{spec}: {parsed:?}"
        );
        if let Ok(set) = parsed {
            let native = native.unwrap();
            for probes in ["fs", "nativeFs"] {
                if let Some(probes) = case[probes].as_array() {
                    for probe in probes {
                        let path = probe[1].as_str().unwrap().into();
                        let operation = match probe[0].as_str().unwrap() {
                            "fs.read" => Operation::FsRead { path },
                            "fs.write" => Operation::FsWrite { path },
                            other => panic!("unknown probe {other}"),
                        };
                        let native_op = match &operation {
                            Operation::FsRead { path } => {
                                ibex2::grant::Operation::FsRead { path: path.clone() }
                            }
                            Operation::FsWrite { path } => {
                                ibex2::grant::Operation::FsWrite { path: path.clone() }
                            }
                            _ => unreachable!(),
                        };
                        assert_eq!(
                            native.permits(&native_op),
                            set.permits(&operation),
                            "Ibex admission: {spec}: {probe}"
                        );
                        assert_eq!(
                            set.permits(&operation),
                            probe[2].as_bool().unwrap(),
                            "{spec}: {probe}"
                        );
                    }
                }
            }
            if let Some(fetches) = case["fetch"].as_array() {
                for probe in fetches {
                    let url = url::Url::parse(probe[0].as_str().unwrap()).unwrap();
                    let origin = Origin::new(
                        url.scheme(),
                        url.host_str().unwrap(),
                        url.port_or_known_default().unwrap(),
                    );
                    assert_eq!(
                        native.permits(&ibex2::grant::Operation::Fetch {
                            origin: ibex2::grant::Origin::new(
                                &origin.scheme,
                                &origin.host,
                                origin.port
                            ),
                        }),
                        set.permits(&Operation::Fetch {
                            origin: origin.clone()
                        }),
                        "Ibex admission: {spec}: {probe}",
                    );
                    assert_eq!(
                        set.permits(&Operation::Fetch { origin }),
                        probe[1].as_bool().unwrap(),
                        "{spec}: {probe}"
                    );
                }
            }
        }
    }
}
