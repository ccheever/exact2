use exact_grants::{GrantSet, Operation, Origin};

#[test]
fn native_and_browser_use_the_same_grant_corpus() {
    let cases: serde_json::Value =
        serde_json::from_str(include_str!("../../host/web/tests/fixtures/grants.json")).unwrap();
    for case in cases.as_array().unwrap() {
        let spec = case["spec"].as_str().unwrap();
        let parsed = GrantSet::parse(spec);
        assert_eq!(
            parsed.is_ok(),
            case["ok"].as_bool().unwrap(),
            "{spec}: {parsed:?}"
        );
        if let Ok(set) = parsed {
            if let Some(fetches) = case["fetch"].as_array() {
                for probe in fetches {
                    let url = url::Url::parse(probe[0].as_str().unwrap()).unwrap();
                    let origin = Origin::new(
                        url.scheme(),
                        url.host_str().unwrap(),
                        url.port_or_known_default().unwrap(),
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
